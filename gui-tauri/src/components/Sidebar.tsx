import type { NavSection, NavSectionId } from "../types/navigation";
import { NavSectionIcon } from "./NavIcons";

interface SidebarProps {
  items: readonly NavSection[];
  activeId: NavSectionId;
  collapsed: boolean;
  onSelect: (id: NavSectionId) => void;
}

const ACTIVE_SURFACE: Record<NavSectionId, string> = {
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
  return (
    <aside
      aria-label="Primary navigation"
      className={`flex shrink-0 flex-col justify-between border-r border-[#1c212b] bg-[#13161d] transition-[width] duration-150 select-none ${
        collapsed ? "w-[88px]" : "w-52"
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
              className={`group relative flex w-full h-[74px] items-center transition-all duration-150 active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
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
                className={`h-9 w-9 shrink-0 transition-colors duration-150 ${
                  isActive
                    ? "text-[#9be000]"
                    : "text-white/90 group-hover:text-white"
                }`}
              />

              {!collapsed && (
                <span
                  className={`truncate text-[13px] tracking-wide ${isActive ? "font-semibold text-white" : "font-medium text-slate-200"
                    }`}
                >
                  {item.label}
                </span>
              )}
            </button>
          );
        })}
      </nav>

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
    </aside>
  );
}
