import { useEffect, useRef, useState, type MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AboutModal } from "./AboutModal";
import {
  CloseIcon,
  InfoIcon,
  MaximizeIcon,
  MenuIcon,
  MinimizeIcon,
  RestoreIcon,
} from "./NavIcons";

interface AppHeaderProps {
  sidebarCollapsed: boolean;
  isMaximized: boolean;
  appVersion: string;
  onToggleSidebar: () => void;
}

export function AppHeader({
  sidebarCollapsed,
  isMaximized,
  appVersion,
  onToggleSidebar,
}: AppHeaderProps) {
  const [modalOpen, setModalOpen] = useState(false);
  const infoWrapRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const handleSetModal = (e: CustomEvent<boolean>) => {
      setModalOpen(e.detail);
    };
    window.addEventListener("uxf-set-modal-open" as any, handleSetModal as any);
    return () => window.removeEventListener("uxf-set-modal-open" as any, handleSetModal as any);
  }, []);

  const stopDragPropagation = (event: MouseEvent<HTMLElement>) => {
    event.stopPropagation();
  };

  const handleMinimize = () => {
    void getCurrentWindow()
      .minimize()
      .catch((err) => console.error("minimize() failed:", err));
  };

  const handleToggleMaximize = () => {
    void getCurrentWindow()
      .toggleMaximize()
      .catch((err) => console.error("toggleMaximize() failed:", err));
  };

  const handleClose = () => {
    void getCurrentWindow()
      .close()
      .catch((err) => console.error("close() failed:", err));
  };

  return (
    <header
      data-tauri-drag-region="deep"
      className="relative z-30 flex h-10 shrink-0 items-center justify-between border-b border-[#1c212b] bg-[#090c11] select-none"
    >
      {/* Left slot aligned cleanly above the navigation rail */}
      <div
        className={`flex h-full shrink-0 items-center border-r border-[#1c212b] bg-[#13161d] transition-[width] duration-150 ${
          sidebarCollapsed ? "w-[88px] justify-center" : "w-52 justify-between px-3"
        }`}
      >
        <button
          type="button"
          onMouseDown={stopDragPropagation}
          onClick={onToggleSidebar}
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!sidebarCollapsed}
          title={sidebarCollapsed ? "Expand navigation" : "Collapse navigation"}
          className="flex h-11 w-11 items-center justify-center rounded text-white/90 transition-all duration-100 hover:bg-[#1b202b] hover:text-[#9be000] active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
        >
          <MenuIcon className="h-9 w-9" />
        </button>

        {!sidebarCollapsed && (
          <span
            data-tauri-drag-region="deep"
            className="text-[11px] font-semibold tracking-[0.16em] text-[#6b7280] uppercase"
          >
            UXF
          </span>
        )}
      </div>

      {/* Centered Title (draggable across full middle span) */}
      <div
        data-tauri-drag-region="deep"
        className="flex h-full flex-1 items-center justify-center px-4"
      >
        <h1 className="pointer-events-none text-[13px] font-medium tracking-[0.04em] text-slate-100">
          Control Center
        </h1>
      </div>

      {/* Right Reference Utility & Window Controls: [i] (+ Popover)  [Min] [Max/Restore] [Close] */}
      <div className="flex h-full shrink-0 items-center gap-1 pr-2 pt-0.5">
        <div ref={infoWrapRef} className="relative mr-2 flex items-center">
          <button
            type="button"
            onMouseDown={stopDragPropagation}
            onClick={() => setModalOpen(true)}
            aria-label="Application information & health"
            aria-expanded={modalOpen}
            aria-haspopup="dialog"
            title="System Health, Updates & Info"
            className={`flex h-7 w-7 items-center justify-center rounded border transition-all duration-100 active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
              modalOpen
                ? "border-[#262b36] bg-[#171c25] text-[#00d8ff]"
                : "border-transparent text-slate-300 hover:border-[#262b36] hover:bg-[#171c25] hover:text-[#00d8ff]"
            }`}
          >
            <InfoIcon className="h-4 w-4" />
          </button>
        </div>

        <AboutModal
          open={modalOpen}
          appVersion={appVersion}
          onClose={() => setModalOpen(false)}
        />

        <button
          type="button"
          onMouseDown={stopDragPropagation}
          onClick={handleMinimize}
          aria-label="Minimize window"
          title="Minimize"
          className="flex h-7 w-7 items-center justify-center rounded text-slate-300 transition-all duration-100 hover:bg-[#191f2a] hover:text-white active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
        >
          <MinimizeIcon className="h-3.5 w-3.5" />
        </button>

        <button
          type="button"
          onMouseDown={stopDragPropagation}
          onClick={handleToggleMaximize}
          aria-label={isMaximized ? "Restore window" : "Maximize window"}
          title={isMaximized ? "Restore" : "Maximize"}
          className="flex h-7 w-7 items-center justify-center rounded text-slate-300 transition-all duration-100 hover:bg-[#191f2a] hover:text-white active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff]"
        >
          {isMaximized ? (
            <RestoreIcon className="h-3.5 w-3.5" />
          ) : (
            <MaximizeIcon className="h-3.5 w-3.5" />
          )}
        </button>

        <button
          type="button"
          onMouseDown={stopDragPropagation}
          onClick={handleClose}
          aria-label="Close window"
          title="Close"
          className="flex h-7 w-7 items-center justify-center rounded text-slate-300 transition-all duration-100 hover:bg-[#c42b1c]/80 hover:text-white active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-red-400"
        >
          <CloseIcon className="h-3.5 w-3.5" />
        </button>
      </div>
    </header>
  );
}
