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
import { SystemCenterPage } from "./components/system-center/SystemCenterPage";
import { useUpdateChecker } from "./hooks/useUpdateChecker";
import {
  DEFAULT_SECTION,
  NAV_SECTIONS,
  type NavSectionId,
  type SystemCenterTabId,
} from "./types/navigation";

export function App() {
  const [activeSectionId, setActiveSectionId] =
    useState<NavSectionId>(DEFAULT_SECTION);
  const [systemCenterTab, setSystemCenterTab] =
    useState<SystemCenterTabId>("overview");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  // Default to RPM release version 2.0.2-4
  const [appVersion, setAppVersion] = useState("2.0.2-4");

  const { updateInfo, showPopup, dismissPopup } = useUpdateChecker(appVersion);

  useEffect(() => {
    let active = true;
    let unlistenResize: (() => void) | undefined;

    const initWindowState = async () => {
      try {
        const version = await invoke<string>("get_app_version");
        if (active && version && version !== "1.0.0") {
          setAppVersion(version);
        }
      } catch (err) {
        try {
          const tauriVer = await getVersion();
          if (active && tauriVer && tauriVer !== "1.0.0") {
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

  // Global keyboard shortcut: Ctrl+I opens / toggles UXF System Center
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "i") {
        event.preventDefault();
        setActiveSectionId((prev) =>
          prev === "system-center" ? "system" : "system-center"
        );
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
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

      // 2. System Center — Overview
      setActiveSectionId("system-center");
      setSystemCenterTab("overview");
      await sleep(1200);
      await notifyStep("sc-overview");

      // 3. System Center — Health
      setSystemCenterTab("health");
      await sleep(1500);
      await notifyStep("sc-health");

      // 4. System Center — Updates
      setSystemCenterTab("updates");
      await sleep(1800);
      await notifyStep("sc-updates");

      // 5. System Center — System Info
      setSystemCenterTab("system-info");
      await sleep(1200);
      await notifyStep("sc-system-info");

      // 6. System Center — About
      setSystemCenterTab("about");
      await sleep(1000);
      await notifyStep("sc-about");

      // Return to Dashboard and signal completion
      setActiveSectionId("system");
      await sleep(400);
      await notifyStep("DONE");
    };

    void runCaptureSequence();

    return () => {
      unmounted = true;
    };
  }, []);

  const handleSelectSection = (id: NavSectionId) => {
    setActiveSectionId(id);
  };

  const activePlaceholderSection =
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
        onToggleSidebar={() => setSidebarCollapsed((prev) => !prev)}
      />

      <div className="flex min-h-0 flex-1">
        <Sidebar
          items={NAV_SECTIONS}
          activeId={activeSectionId}
          collapsed={sidebarCollapsed}
          onSelect={handleSelectSection}
        />

        <main className="min-w-0 flex-1 overflow-x-hidden overflow-y-auto bg-[#0c0f14] select-none">
          <div key={activeSectionId} className="uxf-tab-fade h-full min-w-0 w-full">
            {activeSectionId === "system" ? (
              <SystemDashboard />
            ) : activeSectionId === "keyboard" ? (
              <KeyboardPage />
            ) : activeSectionId === "system-center" ? (
              <SystemCenterPage
                appVersion={appVersion}
                activeTab={systemCenterTab}
                onSelectTab={setSystemCenterTab}
                onBackToDashboard={() => setActiveSectionId("system")}
              />
            ) : (
              <div className="h-full p-5 sm:p-7">
                <ViewPlaceholder section={activePlaceholderSection} />
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
