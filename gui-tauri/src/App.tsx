import { useEffect, useState } from "react";
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
  // Default to RPM release version 2.0.2
  const [appVersion, setAppVersion] = useState("2.0.2");

  const { updateInfo, showPopup, dismissPopup } = useUpdateChecker(appVersion);

  useEffect(() => {
    let active = true;
    let unlistenResize: (() => void) | undefined;

    const initWindowState = async () => {
      try {
        const version = await getVersion();
        if (active && version) {
          setAppVersion(version);
        }
      } catch (err) {
        console.warn("getVersion() unavailable outside Tauri runtime:", err);
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
