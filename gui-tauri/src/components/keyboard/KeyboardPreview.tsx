import type { CSSProperties } from "react";

interface KeyCapDef {
  label: string;
  secondary?: string;
  span?: number;
  alignLeft?: boolean;
  icon?: "backspace" | "enter" | "menu" | "up" | "down" | "left" | "right";
}

// Reference top function row (16 compact keys flush across 15u main block width)
const MAIN_FN_ROW: readonly KeyCapDef[] = [
  { label: "Esc" },
  { label: "F1" },
  { label: "F2" },
  { label: "F3" },
  { label: "F4" },
  { label: "F5" },
  { label: "F6" },
  { label: "F7" },
  { label: "F8" },
  { label: "F9" },
  { label: "F10" },
  { label: "F11" },
  { label: "F12" },
  { label: "PrtSc" },
  { label: "Pause" },
  { label: "Del" },
];

const NUMPAD_FN_ROW: readonly KeyCapDef[] = [
  { label: "Home" },
  { label: "End" },
  { label: "PgUp" },
  { label: "PgDn" },
];

// Rows 2-6 of Main Block matching exact reference key arrangement (sum = 15.0 per row)
const MAIN_BODY_ROWS: readonly (readonly KeyCapDef[])[] = [
  [
    { label: "`", secondary: "~", span: 1 },
    { label: "1", span: 1 },
    { label: "2", span: 1 },
    { label: "3", span: 1 },
    { label: "4", span: 1 },
    { label: "5", span: 1 },
    { label: "6", span: 1 },
    { label: "7", span: 1 },
    { label: "8", span: 1 },
    { label: "9", span: 1 },
    { label: "0", span: 1 },
    { label: "-", secondary: "_", span: 1 },
    { label: "=", secondary: "+", span: 1 },
    { label: "Backspace", icon: "backspace", span: 2 },
  ],
  [
    { label: "Tab", secondary: "⇄", span: 1.5, alignLeft: true },
    { label: "Q", span: 1 },
    { label: "W", span: 1 },
    { label: "E", span: 1 },
    { label: "R", span: 1 },
    { label: "T", span: 1 },
    { label: "Y", span: 1 },
    { label: "U", span: 1 },
    { label: "I", span: 1 },
    { label: "O", span: 1 },
    { label: "P", span: 1 },
    { label: "[", secondary: "{", span: 1 },
    { label: "]", secondary: "}", span: 1 },
    { label: "\\", secondary: "|", span: 1.5 },
  ],
  [
    { label: "Caps Lock", span: 1.75, alignLeft: true },
    { label: "A", span: 1 },
    { label: "S", span: 1 },
    { label: "D", span: 1 },
    { label: "F", span: 1 },
    { label: "G", span: 1 },
    { label: "H", span: 1 },
    { label: "J", span: 1 },
    { label: "K", span: 1 },
    { label: "L", span: 1 },
    { label: ";", secondary: ":", span: 1 },
    { label: "'", secondary: "\"", span: 1 },
    { label: "Enter", icon: "enter", span: 2.25 },
  ],
  [
    { label: "⇧ Shift", span: 2.25, alignLeft: true },
    { label: "Z", span: 1 },
    { label: "X", span: 1 },
    { label: "C", span: 1 },
    { label: "V", span: 1 },
    { label: "B", span: 1 },
    { label: "N", span: 1 },
    { label: "M", span: 1 },
    { label: ",", secondary: "<", span: 1 },
    { label: ".", secondary: ">", span: 1 },
    { label: "/", secondary: "?", span: 1 },
    { label: "⇧ Shift", span: 1.75 },
    { label: "↑", icon: "up", span: 1 },
  ],
  [
    { label: "Ctrl", span: 1.25 },
    { label: "Fn", span: 1 },
    { label: "SUP", span: 1.15 },
    { label: "Alt", span: 1.1 },
    { label: "", span: 5.0 },
    { label: "Alt", span: 1 },
    { label: "Menu", icon: "menu", span: 1 },
    { label: "Ctrl", span: 1.5 },
    { label: "←", icon: "left", span: 1 },
    { label: "↓", icon: "down", span: 1 },
  ],
];

const NUMPAD_ROW_2: readonly KeyCapDef[] = [
  { label: "NumLk" },
  { label: "/" },
  { label: "*" },
  { label: "-" },
];

export interface KeyboardPreviewProps {
  color: [number, number, number];
  brightnessStep: number;
  breathing?: boolean;
  dimmed?: boolean;
}

function renderKeyIcon(icon: KeyCapDef["icon"]) {
  switch (icon) {
    case "backspace":
      return (
        <svg viewBox="0 0 18 10" className="h-2.5 w-4" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
          <path d="M16 5H3M6 2L2.5 5 6 8" />
        </svg>
      );
    case "enter":
      return (
        <svg viewBox="0 0 18 12" className="h-3 w-4" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
          <path d="M15 2v5H3.5M6.5 4.2L3 7l3.5 2.8" />
        </svg>
      );
    case "menu":
      return (
        <svg viewBox="0 0 12 12" className="h-2.5 w-2.5" fill="none" stroke="currentColor" strokeWidth="1.2">
          <rect x="2" y="1.5" width="8" height="9" rx="0.8" />
          <path d="M4 4.2h4M4 6.2h4M4 8.2h2.5" strokeLinecap="round" />
        </svg>
      );
    case "up":
      return <span className="text-[7px] leading-none">▲</span>;
    case "down":
      return <span className="text-[7px] leading-none">▼</span>;
    case "left":
      return <span className="text-[7px] leading-none">◀</span>;
    case "right":
      return <span className="text-[7px] leading-none">▶</span>;
    default:
      return null;
  }
}

export function KeyboardPreview({
  color,
  brightnessStep,
  breathing = false,
  dimmed = false,
}: KeyboardPreviewProps) {
  const [r, g, b] = color;
  const clampedStep = dimmed ? 0 : Math.max(0, Math.min(4, brightnessStep));
  const isLit = clampedStep > 0;
  const levelFactor = [0, 0.34, 0.56, 0.78, 1.0][clampedStep];

  const glowBorderAlpha = isLit ? 0.24 + levelFactor * 0.62 : 0.16;
  const glowTextAlpha = isLit ? 0.45 + levelFactor * 0.52 : 0.36;
  const glowHaloAlpha = isLit ? 0.12 + levelFactor * 0.44 : 0;
  const glowFillAlpha = isLit ? 0.04 + levelFactor * 0.22 : 0;

  const capGlowStyle: CSSProperties = {
    borderColor: isLit
      ? `rgba(${r}, ${g}, ${b}, ${glowBorderAlpha.toFixed(3)})`
      : "rgba(55, 65, 81, 0.48)",
    color: isLit
      ? `rgba(${r}, ${g}, ${b}, ${glowTextAlpha.toFixed(3)})`
      : "rgba(107, 114, 128, 0.68)",
    background: isLit
      ? `radial-gradient(ellipse at center, rgba(${r}, ${g}, ${b}, ${(
          glowFillAlpha * 0.45
        ).toFixed(3)}) 0%, rgba(10, 13, 18, 0.96) 80%)`
      : "linear-gradient(180deg, rgba(16, 20, 27, 0.96) 0%, rgba(10, 12, 17, 0.98) 100%)",
    boxShadow: isLit
      ? `0 0 ${Math.round(4 + levelFactor * 7)}px rgba(${r}, ${g}, ${b}, ${(
          glowHaloAlpha * 0.62
        ).toFixed(3)}), inset 0 0 ${Math.round(3 + levelFactor * 5)}px rgba(${r}, ${g}, ${b}, ${(
          glowFillAlpha * 0.7
        ).toFixed(3)})`
      : "inset 0 1px 0 rgba(255, 255, 255, 0.03)",
  };

  const renderKeyContent = (key: KeyCapDef, compactFn = false) => {
    if (key.icon) {
      return renderKeyIcon(key.icon);
    }
    if (key.secondary) {
      return (
        <span className="flex flex-col items-center justify-center leading-[1.05]">
          <span className="text-[7px] opacity-85">{key.secondary}</span>
          <span className="text-[8.5px]">{key.label}</span>
        </span>
      );
    }
    const textClass = compactFn
      ? "text-[7px] font-medium tracking-tight"
      : key.label === "SUP"
        ? "text-[7.5px] font-semibold tracking-tight"
        : key.label.length > 3
          ? "text-[7.5px] font-medium tracking-tight"
          : "text-[9px] font-medium";
    return <span className={textClass}>{key.label}</span>;
  };

  const keyBaseClass =
    "relative flex select-none items-center justify-center overflow-hidden rounded-[2.5px] border transition-colors duration-150";

  const renderStaticOrGlowingKey = (
    keyId: string,
    key: KeyCapDef,
    extraClass: string,
    style?: CSSProperties,
    compactFn = false
  ) => (
    <div
      key={keyId}
      style={{
        ...style,
        ...(!isLit ? capGlowStyle : {
          borderColor: "rgba(55, 65, 81, 0.36)",
          color: "rgba(107, 114, 128, 0.55)",
          background: "linear-gradient(180deg, rgba(16, 20, 27, 0.96) 0%, rgba(10, 12, 17, 0.98) 100%)",
        }),
      }}
      className={`${keyBaseClass} ${
        key.alignLeft ? "justify-start px-1.5" : "justify-center px-0.5"
      } ${extraClass}`}
    >
      {isLit && (
        <span
          aria-hidden="true"
          style={capGlowStyle}
          className={`pointer-events-none absolute inset-0 flex items-center rounded-[2.5px] border ${
            key.alignLeft ? "justify-start px-1.5" : "justify-center px-0.5"
          } ${breathing ? "uxf-kbd-breathing-layer" : ""}`}
        >
          {renderKeyContent(key, compactFn)}
        </span>
      )}
      {!isLit && renderKeyContent(key, compactFn)}
    </div>
  );

  return (
    <div
      aria-label="Visual backlit laptop keyboard preview"
      className="relative mx-auto w-full max-w-[620px] overflow-hidden rounded-[5px] border border-[#1a202b] bg-[#06080c] p-2 shadow-[0_10px_28px_rgba(0,0,0,0.65)]"
      style={
        isLit
          ? {
              boxShadow: `0 0 ${Math.round(14 + levelFactor * 16)}px rgba(${r}, ${g}, ${b}, ${(
                0.08 +
                levelFactor * 0.22
              ).toFixed(3)}), 0 10px 28px rgba(0, 0, 0, 0.72)`,
            }
          : undefined
      }
    >
      <div className="flex items-stretch gap-1.5">
        {/* Left Main Block (~15u proportional width) */}
        <div className="flex flex-[15] flex-col gap-[3px]">
          {/* Row 1: Compact Fn row (16 flush columns) */}
          <div className="grid grid-cols-16 gap-[3px]">
            {MAIN_FN_ROW.map((k, idx) =>
              renderStaticOrGlowingKey(`mfn-${idx}`, k, "h-[19px]", undefined, true)
            )}
          </div>

          {/* Rows 2-6: Exact staggered proportional widths summing to 15.0 per row */}
          {MAIN_BODY_ROWS.map((row, rIdx) => (
            <div key={`mrow-${rIdx}`} className="flex gap-[3px]">
              {row.map((k, cIdx) =>
                renderStaticOrGlowingKey(
                  `m-${rIdx}-${cIdx}`,
                  k,
                  "h-[31px] min-w-0",
                  { flexGrow: k.span ?? 1, flexBasis: 0 }
                )
              )}
            </div>
          ))}
        </div>

        {/* Right Numpad Block (~4u proportional width) */}
        <div className="flex flex-[4] flex-col gap-[3px]">
          {/* Row 1: Home / End / PgUp / PgDn matching Fn row height */}
          <div className="grid grid-cols-4 gap-[3px]">
            {NUMPAD_FN_ROW.map((k, idx) =>
              renderStaticOrGlowingKey(`nfn-${idx}`, k, "h-[19px]", undefined, true)
            )}
          </div>

          {/* Row 2: NumLk / * - */}
          <div className="grid grid-cols-4 gap-[3px]">
            {NUMPAD_ROW_2.map((k, idx) =>
              renderStaticOrGlowingKey(`nr2-${idx}`, k, "h-[31px]")
            )}
          </div>

          {/* Rows 3-4: 7 8 9 / 4 5 6 + 2-row tall '+' */}
          <div className="grid grid-cols-4 grid-rows-2 gap-[3px] h-[65px]">
            {renderStaticOrGlowingKey("n-7", { label: "7" }, "h-full")}
            {renderStaticOrGlowingKey("n-8", { label: "8" }, "h-full")}
            {renderStaticOrGlowingKey("n-9", { label: "9" }, "h-full")}
            {renderStaticOrGlowingKey("n-plus", { label: "+" }, "row-span-2 h-full")}
            {renderStaticOrGlowingKey("n-4", { label: "4" }, "h-full")}
            {renderStaticOrGlowingKey("n-5", { label: "5" }, "h-full")}
            {renderStaticOrGlowingKey("n-6", { label: "6" }, "h-full")}
          </div>

          {/* Rows 5-6: 1 2 3 / → 0 . + 2-row tall 'Enter' */}
          <div className="grid grid-cols-4 grid-rows-2 gap-[3px] h-[65px]">
            {renderStaticOrGlowingKey("n-1", { label: "1" }, "h-full")}
            {renderStaticOrGlowingKey("n-2", { label: "2" }, "h-full")}
            {renderStaticOrGlowingKey("n-3", { label: "3" }, "h-full")}
            {renderStaticOrGlowingKey("n-ent", { label: "Enter" }, "row-span-2 h-full")}
            {renderStaticOrGlowingKey("n-right", { label: "→", icon: "right" }, "h-full")}
            {renderStaticOrGlowingKey("n-0", { label: "0" }, "h-full")}
            {renderStaticOrGlowingKey("n-dot", { label: "." }, "h-full")}
          </div>
        </div>
      </div>
    </div>
  );
}
