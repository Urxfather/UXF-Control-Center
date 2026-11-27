import {
  useRef,
  type ChangeEvent,
  type PointerEvent,
} from "react";
import { hexToRgb } from "../../hooks/useKeyboardBackend";

export interface WheelSegment {
  label: string;
  hex: string;
  rgb: [number, number, number];
}

// 12-segment spectrum clockwise from top (12 o'clock)
export const WHEEL_SEGMENTS: readonly WheelSegment[] = [
  { label: "Vivid Red", hex: "#ff0000", rgb: [255, 0, 0] },
  { label: "Hot Pink", hex: "#ff007f", rgb: [255, 0, 127] },
  { label: "Magenta", hex: "#e600ff", rgb: [230, 0, 255] },
  { label: "Violet Purple", hex: "#8b2cf5", rgb: [139, 44, 245] },
  { label: "Royal Blue", hex: "#0048ff", rgb: [0, 72, 255] },
  { label: "Cerulean", hex: "#2b78c5", rgb: [43, 120, 197] },
  { label: "Aqua Cyan", hex: "#00c8d4", rgb: [0, 200, 212] },
  { label: "Mint", hex: "#00e68a", rgb: [0, 230, 138] },
  { label: "Neon Green", hex: "#18e018", rgb: [24, 224, 24] },
  { label: "Lime", hex: "#9be000", rgb: [155, 224, 0] },
  { label: "Yellow", hex: "#f5dc00", rgb: [245, 220, 0] },
  { label: "Orange", hex: "#ff7300", rgb: [255, 115, 0] },
] as const;

const CX = 165;
const CY = 165;
const R_OUTER = 115;
const R_INNER = 87;
const SUNBURST_RAYS = 72;

function polar(r: number, deg: number): [number, number] {
  const rad = (deg * Math.PI) / 180;
  return [CX + r * Math.cos(rad), CY + r * Math.sin(rad)];
}

function ringArcPath(
  rInner: number,
  rOuter: number,
  startDeg: number,
  endDeg: number
): string {
  const [ox1, oy1] = polar(rOuter, startDeg);
  const [ox2, oy2] = polar(rOuter, endDeg);
  const [ix2, iy2] = polar(rInner, endDeg);
  const [ix1, iy1] = polar(rInner, startDeg);
  return [
    `M ${ox1.toFixed(2)} ${oy1.toFixed(2)}`,
    `A ${rOuter} ${rOuter} 0 0 1 ${ox2.toFixed(2)} ${oy2.toFixed(2)}`,
    `L ${ix2.toFixed(2)} ${iy2.toFixed(2)}`,
    `A ${rInner} ${rInner} 0 0 0 ${ix1.toFixed(2)} ${iy1.toFixed(2)}`,
    "Z",
  ].join(" ");
}

function rgbHueIndex(rgb: [number, number, number]): number {
  let bestIdx = 0;
  let bestDist = Number.POSITIVE_INFINITY;
  for (let i = 0; i < WHEEL_SEGMENTS.length; i += 1) {
    const s = WHEEL_SEGMENTS[i].rgb;
    const dist =
      (s[0] - rgb[0]) ** 2 + (s[1] - rgb[1]) ** 2 + (s[2] - rgb[2]) ** 2;
    if (dist < bestDist) {
      bestDist = dist;
      bestIdx = i;
    }
  }
  return bestIdx;
}

interface ColorBacklightWheelProps {
  color: [number, number, number];
  colorHex: string;
  disabled: boolean;
  onSelectColor: (rgb: [number, number, number], commit: boolean) => void;
}

export function ColorBacklightWheel({
  color,
  colorHex,
  disabled,
  onSelectColor,
}: ColorBacklightWheelProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const draggingRef = useRef(false);

  const activeIdx = rgbHueIndex(color);
  const activeStartDeg = -90 + activeIdx * 30;
  const activeEndDeg = activeStartDeg + 30;
  const [m1x, m1y] = polar(R_OUTER + 3.5, activeStartDeg + 2);
  const [m2x, m2y] = polar(R_OUTER + 3.5, activeEndDeg - 2);

  const segmentFromPointer = (clientX: number, clientY: number): WheelSegment | null => {
    const svg = svgRef.current;
    if (!svg) {
      return null;
    }
    const rect = svg.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return null;
    }
    const x = ((clientX - rect.left) / rect.width) * 330 - CX;
    const y = ((clientY - rect.top) / rect.height) * 330 - CY;
    const dist = Math.hypot(x, y);
    if (dist < R_INNER - 6 || dist > R_OUTER + 18) {
      return null;
    }
    const deg = ((Math.atan2(y, x) * 180) / Math.PI + 90 + 360) % 360;
    const idx = Math.floor(deg / 30) % WHEEL_SEGMENTS.length;
    return WHEEL_SEGMENTS[idx] ?? null;
  };

  const handlePointerDown = (event: PointerEvent<SVGSVGElement>) => {
    if (disabled || event.button !== 0) {
      return;
    }
    const seg = segmentFromPointer(event.clientX, event.clientY);
    if (!seg) {
      return;
    }
    event.preventDefault();
    draggingRef.current = true;
    event.currentTarget.setPointerCapture(event.pointerId);
    onSelectColor(seg.rgb, false);
  };

  const handlePointerMove = (event: PointerEvent<SVGSVGElement>) => {
    if (disabled || !draggingRef.current) {
      return;
    }
    const seg = segmentFromPointer(event.clientX, event.clientY);
    if (seg) {
      onSelectColor(seg.rgb, false);
    }
  };

  const finishDrag = (event: PointerEvent<SVGSVGElement>) => {
    if (disabled || !draggingRef.current) {
      return;
    }
    draggingRef.current = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    const seg = segmentFromPointer(event.clientX, event.clientY);
    onSelectColor(seg ? seg.rgb : color, true);
  };

  const handleCustomHexChange = (event: ChangeEvent<HTMLInputElement>) => {
    if (disabled) {
      return;
    }
    onSelectColor(hexToRgb(event.target.value), true);
  };

  return (
    <div className="relative -my-4 flex h-[300px] w-[300px] items-center justify-center select-none lg:h-[330px] lg:w-[330px]">
      {/* Ambient dynamic radial sunburst glow driven phase-accurately via CSS variables */}
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-[-12px] rounded-full"
        style={{
          opacity: "var(--uxf-wheel-ambient-op, 0)",
          background: "var(--uxf-wheel-bg, none)",
        }}
      />

      <svg
        ref={svgRef}
        viewBox="0 0 330 330"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishDrag}
        onPointerCancel={finishDrag}
        aria-label="Color Backlight wheel"
        className={`relative z-10 h-full w-full ${
          disabled ? "cursor-not-allowed opacity-60" : "cursor-pointer"
        }`}
      >
        <defs>
          <radialGradient
            id="uxf-sunburst-mask-grad"
            cx={CX}
            cy={CY}
            r={163}
            gradientUnits="userSpaceOnUse"
          >
            <stop offset="68%" stopColor="#ffffff" stopOpacity="0.95" />
            <stop offset="87%" stopColor="#ffffff" stopOpacity="0.45" />
            <stop offset="100%" stopColor="#ffffff" stopOpacity="0.06" />
          </radialGradient>
          <mask id="uxf-sunburst-mask">
            <rect x="0" y="0" width="330" height="330" fill="url(#uxf-sunburst-mask-grad)" />
          </mask>
        </defs>

        {/* Outer radial dotted sunburst rays */}
        <g
          mask="url(#uxf-sunburst-mask)"
          style={{ opacity: "var(--uxf-ray-op, 0.45)" }}
        >
          {Array.from({ length: SUNBURST_RAYS }, (_, idx) => {
            const angle = (idx / SUNBURST_RAYS) * 360;
            const [x1, y1] = polar(121, angle);
            const [x2, y2] = polar(163, angle);
            return (
              <line
                key={idx}
                x1={x1.toFixed(2)}
                y1={y1.toFixed(2)}
                x2={x2.toFixed(2)}
                y2={y2.toFixed(2)}
                stroke="var(--uxf-ray-color, #252c39)"
                strokeWidth="1.45"
                strokeLinecap="round"
                strokeDasharray="1.5 4.2"
              />
            );
          })}
        </g>

        {/* Outer dark bezel ring */}
        <circle
          cx={CX}
          cy={CY}
          r={R_OUTER + 5}
          fill="#070a0f"
          stroke="#181f2b"
          strokeWidth="1.1"
        />

        {/* 12 vivid spectrum ring wedges */}
        {WHEEL_SEGMENTS.map((seg, idx) => {
          const a0 = -90 + idx * 30;
          const a1 = a0 + 30;
          return (
            <path
              key={seg.hex}
              d={ringArcPath(R_INNER, R_OUTER, a0, a1)}
              fill={seg.hex}
              stroke="#080b10"
              strokeWidth="0.7"
              aria-label={seg.label}
              className="transition-opacity duration-150 hover:opacity-95"
            />
          );
        })}

        {/* Active color arc selection indicator */}
        <path
          d={`M ${m1x.toFixed(2)} ${m1y.toFixed(2)} A ${(R_OUTER + 3.5).toFixed(1)} ${(
            R_OUTER + 3.5
          ).toFixed(1)} 0 0 1 ${m2x.toFixed(2)} ${m2y.toFixed(2)}`}
          fill="none"
          stroke="#ffffff"
          strokeWidth="2.6"
          strokeLinecap="round"
        />

        {/* Inner dark cutout circle */}
        <circle
          cx={CX}
          cy={CY}
          r={R_INNER - 1.5}
          fill="#070a0f"
          stroke="#1a212e"
          strokeWidth="1.1"
        />
      </svg>

      {/* Center Label + Custom 16.7M RGB picker pill */}
      <div className="pointer-events-none absolute inset-0 z-20 flex flex-col items-center justify-center gap-1.5">
        <span className="text-[14px] font-medium tracking-[0.02em] text-[#e5e7eb]">
          Color Backlight
        </span>
        <label
          title="Select custom 16.7M RGB backlight color"
          className={`pointer-events-auto flex items-center gap-1.5 rounded border border-[#1f2633] bg-[#0b0e14]/90 px-2 py-0.5 text-[11px] font-mono text-[#9ca3af] uppercase transition-colors hover:border-[#374151] hover:text-slate-200 ${
            disabled ? "pointer-events-none opacity-50" : "cursor-pointer"
          }`}
        >
          <span
            className="h-2.5 w-2.5 rounded-[2px] border border-white/25"
            style={{ backgroundColor: colorHex }}
          />
          <span>{colorHex}</span>
          <input
            type="color"
            value={colorHex}
            disabled={disabled}
            onChange={handleCustomHexChange}
            aria-label="Custom RGB backlight color"
            className="sr-only"
          />
        </label>
      </div>
    </div>
  );
}
