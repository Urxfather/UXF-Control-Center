use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};
use zbus::export::futures_util::StreamExt;
use zbus::message::Type as MessageType;
use zbus::{Connection, MatchRule, MessageStream, Proxy};

const UPOWER_DEST: &str = "org.freedesktop.UPower";
const UPOWER_KBD_PATH: &str = "/org/freedesktop/UPower/KbdBacklight";
const UPOWER_KBD_DEV_PATH: &str = "/org/freedesktop/UPower/KbdBacklight/rgbokbd_backlight";
const UPOWER_KBD_IFACE: &str = "org.freedesktop.UPower.KbdBacklight";
const UPOWER_CALL_TIMEOUT_MS: u64 = 1200;

/// Shared System D-Bus helper for reading and updating UPower's keyboard backlight object.
pub struct UPowerBridge {
    conn: AsyncMutex<Option<Connection>>,
}

impl UPowerBridge {
    pub fn new() -> Self {
        Self {
            conn: AsyncMutex::new(None),
        }
    }

    async fn get_connection(&self) -> Result<Connection, String> {
        let mut guard = self.conn.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(existing.clone());
        }
        let created = Connection::system()
            .await
            .map_err(|e| format!("Failed connecting to System D-Bus: {e}"))?;
        *guard = Some(created.clone());
        Ok(created)
    }

    async fn drop_connection(&self) {
        let mut guard = self.conn.lock().await;
        *guard = None;
    }

    pub async fn get_brightness(&self) -> Result<u8, String> {
        let conn = self.get_connection().await?;
        let res = async {
            let proxy = Proxy::new(&conn, UPOWER_DEST, UPOWER_KBD_PATH, UPOWER_KBD_IFACE)
                .await
                .map_err(|e| format!("UPower proxy failed: {e}"))?;
            let val: i32 = proxy
                .call("GetBrightness", &())
                .await
                .map_err(|e| format!("UPower GetBrightness failed: {e}"))?;
            Ok::<u8, String>(val.clamp(0, 255) as u8)
        };
        match tokio::time::timeout(Duration::from_millis(UPOWER_CALL_TIMEOUT_MS), res).await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => {
                self.drop_connection().await;
                Err(e)
            }
            Err(_) => {
                self.drop_connection().await;
                Err("UPower GetBrightness timed out".into())
            }
        }
    }

    pub async fn set_brightness(&self, value: u8) -> Result<(), String> {
        let conn = self.get_connection().await?;
        let res = async {
            let proxy = Proxy::new(&conn, UPOWER_DEST, UPOWER_KBD_PATH, UPOWER_KBD_IFACE)
                .await
                .map_err(|e| format!("UPower proxy failed: {e}"))?;
            proxy
                .call::<_, _, ()>("SetBrightness", &(value as i32))
                .await
                .map_err(|e| format!("UPower SetBrightness failed: {e}"))?;
            Ok::<(), String>(())
        };
        match tokio::time::timeout(Duration::from_millis(UPOWER_CALL_TIMEOUT_MS), res).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                self.drop_connection().await;
                Err(e)
            }
            Err(_) => {
                self.drop_connection().await;
                Err("UPower SetBrightness timed out".into())
            }
        }
    }

    /// Spawns a dedicated async UPower command actor inside the active Tokio runtime.
    pub fn spawn_actor(self: &Arc<Self>) -> UPowerSyncHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        let bridge = Arc::clone(self);
        tokio::spawn(async move {
            run_upower_actor_loop(rx, move |val| {
                let b = Arc::clone(&bridge);
                async move { b.set_brightness(val).await }
            })
            .await;
        });
        UPowerSyncHandle { tx }
    }
}

#[derive(Debug)]
enum UPowerActorCmd {
    SetBrightness(u8),
    Flush(oneshot::Sender<Result<(), String>>),
}

/// Non-blocking handle safe to call from any sync or async thread without `block_on`.
#[derive(Clone)]
pub struct UPowerSyncHandle {
    tx: mpsc::UnboundedSender<UPowerActorCmd>,
}

impl UPowerSyncHandle {
    /// Creates an inert handle when running outside an async actor (e.g., synchronous unit tests).
    #[allow(dead_code)]
    pub fn disconnected() -> Self {
        let (tx, _rx) = mpsc::unbounded_channel();
        Self { tx }
    }

    /// Enqueues a UPower brightness sync cleanly without blocking or calling `block_on`.
    pub fn set_brightness_nonblocking(&self, value: u8) -> Result<(), String> {
        self.tx
            .send(UPowerActorCmd::SetBrightness(value))
            .map_err(|_| "UPower async actor unavailable".into())
    }

    /// Awaits completion of all previously queued UPower brightness writes from an async handler.
    pub async fn flush(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self.tx.send(UPowerActorCmd::Flush(reply_tx)).is_err() {
            return Err("UPower async actor unavailable".into());
        }
        match tokio::time::timeout(Duration::from_millis(1600), reply_rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("UPower sync confirmation dropped".into()),
            Err(_) => Err("UPower sync flush timed out".into()),
        }
    }
}

async fn run_upower_actor_loop<F, Fut>(
    mut rx: mpsc::UnboundedReceiver<UPowerActorCmd>,
    apply_fn: F,
) where
    F: Fn(u8) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<(), String>> + Send,
{
    let mut last_result: Result<(), String> = Ok(());
    let mut pending_flushers: Vec<oneshot::Sender<Result<(), String>>> = Vec::new();

    while let Some(cmd) = rx.recv().await {
        let mut latest_val: Option<u8> = match cmd {
            UPowerActorCmd::SetBrightness(v) => Some(v),
            UPowerActorCmd::Flush(tx) => {
                pending_flushers.push(tx);
                None
            }
        };

        // Coalesce burst brightness updates cleanly before running D-Bus SetBrightness.
        while let Ok(next) = rx.try_recv() {
            match next {
                UPowerActorCmd::SetBrightness(v) => {
                    latest_val = Some(v);
                }
                UPowerActorCmd::Flush(tx) => {
                    pending_flushers.push(tx);
                }
            }
        }

        if let Some(val) = latest_val {
            match apply_fn(val).await {
                Ok(()) => last_result = Ok(()),
                Err(err) => {
                    eprintln!("UPower SetBrightness({val}) error: {err}");
                    last_result = Err(err);
                }
            }
        }

        for flusher in pending_flushers.drain(..) {
            let _ = flusher.send(last_result.clone());
        }
    }
}

/// Listens reactively to UPower KbdBacklight change signals on System D-Bus.
pub async fn watch_upower_signals<F>(on_brightness: F)
where
    F: Fn(u8) + Send + Sync + 'static,
{
    let mut backoff_ms = 250_u64;
    loop {
        match run_upower_watch_once(&on_brightness).await {
            Ok(()) => backoff_ms = 250,
            Err(err) => {
                eprintln!("UPower watch reconnect scheduled ({backoff_ms}ms): {err}");
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(4_000);
            }
        }
    }
}

async fn run_upower_watch_once<F>(on_brightness: &F) -> Result<(), String>
where
    F: Fn(u8) + Send + Sync,
{
    let conn = Connection::system()
        .await
        .map_err(|e| format!("System D-Bus connect failed: {e}"))?;
    let dbus_proxy = zbus::fdo::DBusProxy::new(&conn)
        .await
        .map_err(|e| format!("DBusProxy failed: {e}"))?;

    for path in [UPOWER_KBD_PATH, UPOWER_KBD_DEV_PATH] {
        for member in ["BrightnessChanged", "BrightnessChangedWithSource"] {
            if let Ok(rule) = MatchRule::builder()
                .msg_type(MessageType::Signal)
                .interface(UPOWER_KBD_IFACE)
                .and_then(|b| b.member(member))
                .and_then(|b| b.path(path))
                .map(|b| b.build())
            {
                let _ = dbus_proxy.add_match_rule(rule).await;
            }
        }
    }

    let mut stream = MessageStream::from(&conn);
    while let Some(msg_res) = stream.next().await {
        let Ok(msg) = msg_res else {
            return Err("D-Bus message stream error".into());
        };
        let header = msg.header();
        let path = header.path().map(|p| p.as_str()).unwrap_or("");
        let iface = header.interface().map(|i| i.as_str()).unwrap_or("");
        let member = header.member().map(|m| m.as_str()).unwrap_or("");

        // Deduplicate on the canonical alias path.
        if iface != UPOWER_KBD_IFACE || path != UPOWER_KBD_PATH {
            continue;
        }
        if member == "BrightnessChangedWithSource" {
            if let Ok((raw, _source)) = msg.body().deserialize::<(i32, String)>() {
                on_brightness(raw.clamp(0, 255) as u8);
            }
        }
    }

    Err("UPower signal stream ended".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn spawn_test_actor(
        seen: Arc<Mutex<Vec<u8>>>,
        fail_on: Option<u8>,
    ) -> UPowerSyncHandle {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            run_upower_actor_loop(rx, move |val| {
                let seen = Arc::clone(&seen);
                async move {
                    seen.lock().unwrap().push(val);
                    if fail_on == Some(val) {
                        Err("simulated UPower error".into())
                    } else {
                        Ok(())
                    }
                }
            })
            .await;
        });
        UPowerSyncHandle { tx }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn upower_actor_runs_inside_current_thread_runtime_without_panic() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_test_actor(Arc::clone(&seen), None);

        handle.set_brightness_nonblocking(64).unwrap();
        handle.set_brightness_nonblocking(128).unwrap();
        handle.set_brightness_nonblocking(200).unwrap();
        handle.flush().await.expect("flush succeeds");

        let recorded = seen.lock().unwrap().clone();
        assert_eq!(recorded.last().copied(), Some(200));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn upower_sync_works_from_async_spawn_blocking_and_std_threads_without_nested_runtime_panic() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let handle = spawn_test_actor(Arc::clone(&seen), Some(13));

        // 1. Inside Tokio blocking pool thread (where Handle::block_on would previously panic)
        let h_blocking = handle.clone();
        tokio::task::spawn_blocking(move || {
            h_blocking.set_brightness_nonblocking(13).unwrap();
        })
        .await
        .unwrap();
        let err = handle.flush().await;
        assert!(err.is_err(), "UPower errors must propagate cleanly without panicking");

        // 2. Inside plain std::thread
        let h_thread = handle.clone();
        std::thread::spawn(move || {
            h_thread.set_brightness_nonblocking(255).unwrap();
        })
        .join()
        .unwrap();

        handle.flush().await.expect("subsequent update succeeds");
        assert_eq!(seen.lock().unwrap().last().copied(), Some(255));
    }
}
