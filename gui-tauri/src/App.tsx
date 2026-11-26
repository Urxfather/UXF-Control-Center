import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppHeader } from "./components/AppHeader";
import { ResizeHandles } from "./components/ResizeHandles";
import { Sidebar } from "./components/Sidebar";
import { UpdateNotificationModal } from "./components/UpdateNotificationModal";
import { ViewPlaceholder } from "./components/ViewPlaceholder";
import { KeyboardPage } from "./components/keyboard/KeyboardPage";
import { SystemDashboard } from "./components/system/SystemDashboard";
import { useUpdateChecker } from "./hooks/useUpdateChecker";
import {
  DEFAULT_SECTION,
  NAV_SECTIONS,
  type NavSectionId,
} from "./types/navigation";

export function App() {
  const [activeSectionId, setActiveSectionId] =
    useState<NavSectionId>(DEFAULT_SECTION);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  // Default to RPM release version 2.0.2-2
  const [appVersion, setAppVersion] = useState("2.0.2-2");

  const { updateInfo, showPopup, dismissPopup } = useUpdateChecker(appVersion);

  useEffect(() => {
    let active = true;
    let unlistenResize: (() => void) | undefined;

    const initWindowState = async () => {
      try {
        const version = await invoke<string>("get_app_version");
        if (active && version) {
          setAppVersion(version);
        }
      } catch (err) {
        try {
          const tauriVer = await getVersion();
          if (active && tauriVer) {
            setAppVersion(tauriVer);
          }
        } catch {
          console.warn("Version detection unavailable outside Tauri runtime:", err);
        }
      }

      try {
        const win = getCurrentWindow();
        const max = await win.isMaximized();
        if (active) {
          setIsMaximized(max);
        }
        unlistenResize = await win.onResized(async () => {
          try {
            const currentMax = await win.isMaximized();
            if (active) {
              setIsMaximized(currentMax);
            }
          } catch {
            // ignore transient window queries during close
          }
        });
      } catch (err) {
        console.warn("Window state sync unavailable outside Tauri runtime:", err);
      }
    };

    void initWindowState();

    return () => {
      active = false;
      unlistenResize?.();
    };
  }, []);

  useEffect(() => {
    let unmounted = false;
    const runCaptureSequence = async () => {
      const isCapture = await invoke<boolean>("get_capture_mode").catch(() => false);
      if (!isCapture) return;

      const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

      // Initial settle time for daemon telemetry polling and live sensors
      await sleep(3000);
      if (unmounted) return;

      const notifyStep = async (step: string, delayAfter = 1500) => {
        if (unmounted) return;
        await invoke("report_capture_step", { step }).catch(() => {});
        await sleep(delayAfter);
      };

      // 1. Main Dashboard
      setActiveSectionId("system");
      await sleep(800);
      await notifyStep("dashboard");

      // 2. Keyboard - Static RGB
      setActiveSectionId("keyboard");
      await sleep(800);
      window.dispatchEvent(new CustomEvent("uxf-set-keyboard-effect", { detail: "static" }));
      window.dispatchEvent(new CustomEvent("uxf-set-keyboard-color", { detail: { r: 0, g: 216, b: 255 } }));
      await sleep(600);
      await notifyStep("keyboard-rgb-static");

      // 3. Keyboard - Rainbow Wave
      window.dispatchEvent(new CustomEvent("uxf-set-keyboard-effect", { detail: "rainbow" }));
      await sleep(1000);
      await notifyStep("keyboard-rgb-rainbow");

      // 4. Keyboard - Breathing Pulse (stays longer so frames can be captured for GIF)
      window.dispatchEvent(new CustomEvent("uxf-set-keyboard-effect", { detail: "breathing" }));
      await sleep(1000);
      await notifyStep("keyboard-rgb-breathing", 3500);

      // Restore static
      window.dispatchEvent(new CustomEvent("uxf-set-keyboard-effect", { detail: "static" }));
      await sleep(400);

      // 5. Flexikey Coming Soon
      setActiveSectionId("flexikey");
      await sleep(800);
      await notifyStep("flexikey-coming-soon");

      // 6. FlexiCharger Coming Soon
      setActiveSectionId("flexicharger");
      await sleep(800);
      await notifyStep("flexicharger-coming-soon");

      // 7. About Modal - About Tab
      setActiveSectionId("system");
      await sleep(400);
      window.dispatchEvent(new CustomEvent("uxf-set-modal-open", { detail: true }));
      await sleep(400);
      window.dispatchEvent(new CustomEvent("uxf-set-about-tab", { detail: "about" }));
      await sleep(800);
      await notifyStep("about");

      // 8. About Modal - Health Check Tab
      window.dispatchEvent(new CustomEvent("uxf-set-about-tab", { detail: "health" }));
      await sleep(2000); // Give health scan time to execute
      await notifyStep("health-check");

      // 9. About Modal - Updates Tab
      window.dispatchEvent(new CustomEvent("uxf-set-about-tab", { detail: "updates" }));
      await sleep(2500); // Give update check time to query GitHub API
      await notifyStep("update-checker");

      // Close modal
      window.dispatchEvent(new CustomEvent("uxf-set-modal-open", { detail: false }));
      await sleep(400);

      // Signal completion
      await notifyStep("DONE");
    };

    void runCaptureSequence();

    return () => {
      unmounted = true;
    };
  }, []);

  const activeSection =
    NAV_SECTIONS.find((section) => section.id === activeSectionId) ??
    NAV_SECTIONS[0];

  return (
    <div
      className={`flex h-screen w-screen flex-col overflow-hidden bg-[#07090c] text-slate-100 select-none ${
        isMaximized
          ? "rounded-none border-0"
          : "rounded-xl border border-[#1f2630]"
      }`}
    >
      <ResizeHandles disabled={isMaximized} />

      <AppHeader
        sidebarCollapsed={sidebarCollapsed}
        isMaximized={isMaximized}
        appVersion={appVersion}
        onToggleSidebar={() => setSidebarCollapsed((prev) => !prev)}
      />

      <div className="flex min-h-0 flex-1">
        <Sidebar
          items={NAV_SECTIONS}
          activeId={activeSection.id}
          collapsed={sidebarCollapsed}
          onSelect={setActiveSectionId}
        />

        <main className="min-w-0 flex-1 overflow-y-auto bg-[#0c0f14] select-none">
          <div key={activeSection.id} className="uxf-tab-fade h-full">
            {activeSection.id === "system" ? (
              <SystemDashboard />
            ) : activeSection.id === "keyboard" ? (
              <KeyboardPage />
            ) : (
              <div className="h-full p-5 sm:p-7">
                <ViewPlaceholder section={activeSection} />
              </div>
            )}
          </div>
        </main>
      </div>

      {showPopup && updateInfo && (
        <UpdateNotificationModal
          updateInfo={updateInfo}
          onDismiss={dismissPopup}
        />
      )}
    </div>
  );
}

export default App;
