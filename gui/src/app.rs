
use eframe::egui;
use egui::{Color32, RichText, Stroke};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::{about, components, instance, keyboard_idle, libacercontrol};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Dashboard, Cooling, Keyboard, Performance, Battery, Settings }

impl Page {
    const ALL: [Self; 6] = [Self::Dashboard, Self::Cooling, Self::Keyboard, Self::Performance, Self::Battery, Self::Settings];
    fn title(self) -> &'static str { match self {
        Self::Dashboard => "Dashboard", Self::Cooling => "Cooling", Self::Keyboard => "Keyboard RGB",
        Self::Performance => "Performance", Self::Battery => "Battery", Self::Settings => "Settings",
    }}
    fn description(self) -> &'static str { match self {
        Self::Dashboard => "Hardware status and device overview",
        Self::Cooling => "Fan control and thermal telemetry",
        Self::Keyboard => "Backlight color, effects, and inactivity behavior",
        Self::Performance => "System performance controls",
        Self::Battery => "Battery care and charging options",
        Self::Settings => "Application preferences and system information",
    }}
}

use crate::theme::{
    BACKGROUND as APP_BG,
    BORDER,
    PRIMARY_ACCENT as ACCENT,
    PRIMARY_TEXT as TEXT,
    SECONDARY_TEXT as MUTED,
    SURFACE,
};
const SIDEBAR_BG: Color32 = Color32::from_rgb(0x10, 0x13, 0x18);

pub(crate) struct ControlCenter {
    page: Page,
    brightness: u8,
    color: Color32,
    notice: Option<(bool, String, Instant)>,
    fan_mode: components::FanMode,
    fan_mode_confirmed: bool,
    animation_mode: components::AnimationMode,
    animation_mode_confirmed: bool,
    idle_timeout_seconds: u32,
    minimize_to_tray: bool,
    tray_ready: bool,
    idle_controller: Option<keyboard_idle::KeyboardIdleController>,
    about_info: about::AboutInfo,
    update_sender: mpsc::Sender<Result<about::LatestRelease, String>>,
    update_receiver: mpsc::Receiver<Result<about::LatestRelease, String>>,
    update_checking: bool,
    update_status: String,
    latest_release: Option<about::LatestRelease>,
    pub(crate) telemetry_collector: crate::telemetry::TelemetryCollector,
    pub(crate) cpu_model: String,
    pub(crate) gpu_model: String,
    pub(crate) cooling_tab: usize,
    pub(crate) settings_tab: usize,
}

impl ControlCenter {
    fn set_notice(&mut self, succeeded: bool, message: String) {
        self.notice = Some((succeeded, message, Instant::now()));
    }

    pub(crate) fn new(background_ready: bool) -> Self {
        let bus = libacercontrol::dbus::AcerControlDBus::new().ok();
        let brightness = bus
            .as_ref()
            .and_then(|bus| bus.keyboard_brightness().ok())
            .unwrap_or(128);
        let color = bus
            .as_ref()
            .and_then(|bus| bus.keyboard_color().ok())
            .map(|(r, g, b)| Color32::from_rgb(r, g, b))
            .unwrap_or(Color32::from_rgb(50, 140, 255));
        let (fan_mode, fan_mode_confirmed) = bus
            .as_ref()
            .and_then(|bus| bus.fan_max_selected().ok())
            .map(|is_max| {
                (
                    if is_max { components::FanMode::Max } else { components::FanMode::Auto },
                    true,
                )
            })
            .unwrap_or((components::FanMode::Auto, false));
        let (animation_mode, animation_mode_confirmed) = bus
            .as_ref()
            .and_then(|bus| {
                if bus.rainbow_selected().ok()? {
                    Some(components::AnimationMode::Rainbow)
                } else if bus.breathing_selected().ok()? {
                    Some(components::AnimationMode::Breathing)
                } else {
                    Some(components::AnimationMode::Off)
                }
            })
            .map(|mode| (mode, true))
            .unwrap_or((components::AnimationMode::Off, false));
        let idle_timeout_seconds = keyboard_idle::load_timeout();
        let stored_minimize_to_tray = keyboard_idle::load_minimize_to_tray();
        // Keep the saved preference visible even if the tray host is slow to
        // appear during login. Availability is tracked separately by tray_ready.
        let minimize_to_tray = stored_minimize_to_tray;
        let autostart_error = keyboard_idle::set_login_autostart(
            idle_timeout_seconds > 0 || minimize_to_tray,
        )
        .err();
        let tray_ready = background_ready;
        let (update_sender, update_receiver) = mpsc::channel();
        let telemetry_collector = crate::telemetry::TelemetryCollector::new();
        let cpu_model = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|info| {
                for line in info.lines() {
                    if line.starts_with("model name") {
                        return line.split(':').nth(1).map(|s| s.trim().to_string());
                    }
                }
                None
            })
            .unwrap_or_else(|| "Unknown CPU Model".into());
        let gpu_model = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=name", "--format=csv,noheader"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::process::Command::new("lspci")
                    .output()
                    .ok()
                    .and_then(|out| {
                        let text = String::from_utf8_lossy(&out.stdout);
                        for line in text.lines() {
                            if (line.contains("3D controller") || line.contains("VGA compatible controller"))
                                && (line.contains("NVIDIA") || line.contains("AMD") || line.contains("Advanced Micro Devices"))
                            {
                                if let Some(name) = line.split(':').nth(2) {
                                    return Some(name.trim().to_string());
                                }
                            }
                        }
                        None
                    })
            })
            .unwrap_or_else(|| "Discrete GPU Unavailable".to_string());

        let page = match std::env::var("ACC_PAGE").as_deref() {
            Ok("cooling") => Page::Cooling,
            Ok("keyboard") => Page::Keyboard,
            Ok("performance") => Page::Performance,
            Ok("battery") => Page::Battery,
            Ok("settings") => Page::Settings,
            _ => Page::Dashboard,
        };

        let settings_tab = match std::env::var("ACC_SETTINGS_TAB").as_deref() {
            Ok("1") | Ok("sysinfo") => 1,
            Ok("2") | Ok("drivers") => 2,
            Ok("3") | Ok("diagnostics") => 3,
            _ => 0,
        };

        Self {
            page,
            brightness,
            color,
            notice: autostart_error.map(|error| {
                (false, format!("Could not configure login startup for keyboard inactivity: {error}"), Instant::now())
            }).or_else(|| (!tray_ready && stored_minimize_to_tray).then(|| (false, "Minimize to tray remains enabled, but the tray service is unavailable. The window may stay open until the desktop tray is ready.".to_owned(), Instant::now()))),
            fan_mode,
            fan_mode_confirmed,
            animation_mode,
            animation_mode_confirmed,
            idle_timeout_seconds,
            minimize_to_tray,
            tray_ready,
            idle_controller: (!background_ready && idle_timeout_seconds > 0).then(|| {
                keyboard_idle::KeyboardIdleController::new(idle_timeout_seconds, brightness)
            }),
            about_info: about::AboutInfo::collect(),
            update_sender,
            update_receiver,
            update_checking: false,
            update_status: "Updates are checked only when you ask.".to_owned(),
            latest_release: None,
            telemetry_collector,
            cpu_model,
            gpu_model,
            cooling_tab: 0,
            settings_tab,
        }
    }

    fn action(&mut self, label: &str, action: impl FnOnce() -> Result<(), String>) -> bool {
        let result = action();
        let succeeded = result.is_ok();
        self.notice = Some(match result { Ok(()) => (true, format!("{label} applied"), Instant::now()), Err(e) => (false, format!("{label} failed: {e}"), Instant::now()) });
        succeeded
    }

    #[allow(dead_code)]
    fn metric(ui: &mut egui::Ui, label: &str, value: Result<i32, String>, unit: &str) {
        let text = value
            .map(|v| format!("{v}{unit}"))
            .unwrap_or_else(|_| "Unavailable".into());

        egui::Frame::new()
            .fill(SURFACE)
            .stroke(Stroke::new(1.0_f32, BORDER))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(12, 12))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                ui.label(
                    RichText::new(label.to_uppercase())
                        .small()
                        .color(MUTED),
                );

                ui.heading(RichText::new(text).size(21.0).color(TEXT));
            });
    }

    #[allow(dead_code)]
    fn metric_grid(ui: &mut egui::Ui, metrics: [(&str, Result<i32, String>, &str); 4]) {
        let available_width = ui.available_width();
        let card_width = (available_width / 2.0).max(1.0);

        for row in 0..2 {
            ui.vertical(|ui| {

                for col in 0..2 {
                    let index = row * 2 + col;
                    let (label, value, unit) = &metrics[index];

                    ui.allocate_ui_with_layout(
                        egui::vec2(card_width, 100.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |card_ui| {
                            card_ui.set_width(card_width);
                            Self::metric(card_ui, label, value.clone(), unit);
                        },
                    );
                }
            });
        }
    }

    pub(crate) fn configure_theme(ctx: &egui::Context) {
        crate::theme::configure_visuals(ctx);
    }
}

mod ui;

impl Drop for ControlCenter {
    fn drop(&mut self) {
        if !self.minimize_to_tray && self.idle_timeout_seconds == 0 {
            instance::request_background_quit();
        }
    }
}

impl eframe::App for ControlCenter {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if instance::take_activation_request() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if instance::take_window_quit_request() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            if self.minimize_to_tray && self.tray_ready {
                log::info!("Window closed; background tray service and inactivity controller remain active.");
            } else if self.idle_timeout_seconds > 0 && self.tray_ready {
                log::info!("Window closed; inactivity controller remains active in the tray service.");
            }
        }
        if let Ok(result) = self.update_receiver.try_recv() {
            self.update_checking = false;
            match result {
                Ok(release) => {
                    let current = env!("CARGO_PKG_VERSION");
                    self.update_status = match about::compare_versions(current, &release.tag_name) {
                        Some(std::cmp::Ordering::Less) => format!("Update available: {}", release.tag_name),
                        Some(std::cmp::Ordering::Equal) => format!("You are up to date ({current})."),
                        Some(std::cmp::Ordering::Greater) => format!("This build ({current}) is newer than the latest published release ({}).", release.tag_name),
                        None => format!("Latest published version: {}", release.tag_name),
                    };
                    self.latest_release = Some(release);
                }
                Err(error) => {
                    self.update_status = if error.starts_with("No published release") {
                        error
                    } else {
                        format!("Could not check for updates: {error}")
                    };
                }
            }
        }
        if let Ok(screenshot_dest) = std::env::var("ACC_SCREENSHOT_PATH") {
            for event in &ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let _ = image::save_buffer(
                        &screenshot_dest,
                        image.as_raw(),
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    );
                    std::process::exit(0);
                }
            }
            static FRAMES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let frame_count = FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if frame_count >= 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            ctx.request_repaint();
        }
        egui::SidePanel::left("navigation")
            .resizable(false)
            .default_width(230.0)
            .frame(egui::Frame::new().fill(SIDEBAR_BG).stroke(Stroke::new(1.0_f32, BORDER)).inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("ACER").size(11.0).strong().color(ACCENT));
                ui.add_space(3.0);
                ui.label(RichText::new("Control Center").size(18.0).strong().color(TEXT));
                ui.label(RichText::new("DEVICE MANAGEMENT").size(9.0).color(MUTED));
                ui.add_space(22.0);
                for page in Page::ALL {
                    if components::sidebar_button(ui, page.title(), self.page == page) {
                        self.page = page;
                    }
                    ui.add_space(2.0);
                }
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION"))).small().color(MUTED));
            });
        });
        egui::TopBottomPanel::top("page-header")
            .frame(egui::Frame::new().fill(SIDEBAR_BG).stroke(Stroke::new(1.0_f32, BORDER)).inner_margin(egui::Margin::symmetric(26, 16)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(RichText::new(self.page.title()).size(22.0).color(TEXT));
                        ui.label(RichText::new(self.page.description()).small().color(MUTED));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("LINUX DEVICE UTILITY").small().color(MUTED));
                    });
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(egui::Margin::same(24)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.set_max_width(980.0);
                    self.page_content(ui);
                    if self.notice.as_ref().is_some_and(|(_, _, created)| created.elapsed() >= Duration::from_secs(3)) {
                        self.notice = None;
                    }
                    if let Some((ok, message, _)) = &self.notice {
                        ui.add_space(16.0);
                        let color = if *ok { ACCENT } else { Color32::from_rgb(220, 132, 122) };
                        egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0_f32, BORDER)).corner_radius(8.0).inner_margin(12).show(ui, |ui| {
                            ui.colored_label(color, message);
                        });
                    }
                });
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}
