import type { NavSection, NavSectionId } from "../types/navigation";
import { NavSectionIcon, SystemCenterInfoIcon } from "./NavIcons";

interface SidebarProps {
  items: readonly NavSection[];
  activeId: NavSectionId;
  collapsed: boolean;
  onSelect: (id: NavSectionId) => void;
}

const ACTIVE_SURFACE: Record<Exclude<NavSectionId, "system-center">, string> = {
  system: "bg-[#07090c]",
  keyboard: "bg-[#06090e]",
  flexikey: "bg-[#0c0f14]",
  flexicharger: "bg-[#0c0f14]",
};

export function Sidebar({
  items,
  activeId,
  collapsed,
  onSelect,
}: SidebarProps) {
  const isSystemCenterActive = activeId === "system-center";

  return (
    <aside
      aria-label="Primary navigation"
      className={`flex shrink-0 flex-col justify-between border-r border-[#1c212b] bg-[#13161d] transition-[width] duration-150 select-none ${collapsed ? "w-[88px]" : "w-52"
        }`}
    >
      <nav className="flex flex-col gap-3 pt-2.5">
        {items.map((item) => {
          const isActive = item.id === activeId;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelect(item.id)}
              aria-current={isActive ? "page" : undefined}
              title={collapsed ? item.label : undefined}
              className={`group relative flex h-[74px] w-full items-center transition-all duration-150 active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
                collapsed ? "justify-center px-0" : "gap-3 px-4"
              } ${
                isActive
                  ? `relative z-10 -mr-px pr-px ${ACTIVE_SURFACE[item.id]} text-[#9be000]`
                  : "bg-transparent text-white/90 hover:bg-[#191d26] hover:text-white"
              }`}
            >
              {isActive && (
                <span
                  aria-hidden="true"
                  className="absolute top-0 bottom-0 left-0 w-[3px] bg-[#9be000]"
                />
              )}

              <NavSectionIcon
                id={item.id}
                className={`h-9 w-9 shrink-0 transition-colors duration-150 ${isActive
                  ? "text-[#9be000]"
                  : "text-white/90 group-hover:text-white"
                  }`}
              />

              {!collapsed && (
                <span
                  className={`truncate text-[13px] tracking-wide ${
                    isActive ? "font-semibold text-white" : "font-medium text-slate-200"
                  }`}
                >
                  {item.label}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Bottom section: Option B System Center button + UXF footer */}
      <div className="flex flex-col">
        <div className="relative pb-2.5">
          <button
            type="button"
            onClick={() => onSelect("system-center")}
            aria-current={isSystemCenterActive ? "page" : undefined}
            aria-label="Open UXF System Center (Ctrl+I)"
            title="Open UXF System Center (Ctrl+I)"
            className={`group relative flex w-full items-center transition-all duration-150 active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] px-1.5 ${
              isSystemCenterActive
                ? "z-10 -mr-px bg-[#0a0c10]"
                : "bg-transparent"
            }`}
          >
            {isSystemCenterActive && (
              <span
                aria-hidden="true"
                className="absolute top-1.5 bottom-1.5 left-0 w-[3px] rounded-r bg-gradient-to-b from-[#00d8ff] to-[#9be000] shadow-[0_0_10px_rgba(155,224,0,0.45)]"
              />
            )}

            <div
              className={`flex w-full min-w-0 items-center rounded-xl transition-all duration-150 ${
                collapsed
                  ? "flex-col justify-center gap-1.5 px-1.5 py-2.5"
                  : "justify-between gap-1 px-2 py-2"
              } ${
                isSystemCenterActive
                  ? "bg-transparent text-[#9be000]"
                  : "bg-[#1a1e27]/45 text-[#7d8694] group-hover:bg-[#252b36] group-hover:text-white group-hover:shadow-[0_2px_10px_rgba(0,0,0,0.35)]"
              }`}
            >
              <div
                className={`flex min-w-0 items-center ${
                  collapsed ? "flex-col gap-1.5" : "gap-1.5"
                }`}
              >
                <SystemCenterInfoIcon
                  className={`shrink-0 transition-colors duration-150 ${
                    collapsed ? "h-[22px] w-[22px]" : "h-[18px] w-[18px]"
                  } ${
                    isSystemCenterActive
                      ? "text-[#9be000] drop-shadow-[0_0_6px_rgba(155,224,0,0.35)]"
                      : "text-[#838c99] group-hover:text-white"
                  }`}
                />

                {collapsed ? (
                  <span
                    className={`text-center text-[10.5px] leading-[1.18] font-medium tracking-tight transition-colors duration-150 ${
                      isSystemCenterActive
                        ? "font-semibold text-[#9be000]"
                        : "text-[#838c99] group-hover:text-slate-100"
                    }`}
                  >
                    System
                    <br />
                    Center
                  </span>
                ) : (
                  <span
                    className={`whitespace-nowrap text-[11.5px] font-medium transition-colors duration-150 ${
                      isSystemCenterActive
                        ? "font-semibold text-[#9be000]"
                        : "text-[#9ca3af] group-hover:text-white"
                    }`}
                  >
                    System Center
                  </span>
                )}
              </div>

              {!collapsed && (
                <span
                  className={`shrink-0 rounded px-1 py-0.5 font-mono text-[9px] transition-colors duration-150 ${
                    isSystemCenterActive
                      ? "bg-[#9be000]/15 text-[#9be000]"
                      : "bg-[#12151c] text-[#6b7280] group-hover:bg-[#191e27] group-hover:text-slate-300"
                  }`}
                >
                  Ctrl+I
                </span>
              )}
            </div>

            {/* Option B styled floating tooltip on hover */}
            <div
              role="tooltip"
              className="pointer-events-none absolute top-1/2 left-full z-40 ml-2.5 -translate-y-1/2 whitespace-nowrap rounded-md border border-[#2f3644] bg-[#1d222b] px-2.5 py-1.5 text-left text-[11px] leading-snug text-slate-200 opacity-0 shadow-xl transition-opacity duration-150 group-hover:opacity-100"
            >
              Open UXF System Center
              <span className="ml-1.5 font-mono text-[10px] text-[#9be000]">
                (Ctrl+I)
              </span>
            </div>
          </button>
        </div>

        <div className="border-t border-[#1e2430] px-2 py-2.5">
          {collapsed ? (
            <span
              className="block text-center font-mono text-[10px] text-[#6b7280]"
              title="UXF-Control-Center"
            >
              UXF
            </span>
          ) : (
            <div className="flex flex-col px-1">
              <span className="text-[11px] font-medium text-slate-300">
                UXF-Control-Center
              </span>
              <span className="font-mono text-[10px] text-[#6b7280]">
                Linux Control
              </span>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}
