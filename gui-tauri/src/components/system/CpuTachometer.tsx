import { useId } from "react";
import { useAnimatedNumber } from "../../hooks/useAnimatedNumber";

interface CpuTachometerProps {
  clockMhz: number | null;
  maxGhz?: number;
  utilizationPct: number | null;
  tempC: number | null;
}

function polarToCartesian(cx: number, cy: number, r: number, angleDeg: number) {
  const rad = ((angleDeg - 90) * Math.PI) / 180;
  return {
    x: cx + r * Math.cos(rad),
    y: cy + r * Math.sin(rad),
  };
}

function describeArc(
  cx: number,
  cy: number,
  r: number,
  startDeg: number,
  endDeg: number
) {
  const start = polarToCartesian(cx, cy, r, startDeg);
  const end = polarToCartesian(cx, cy, r, endDeg);
  const largeArc = Math.abs(endDeg - startDeg) <= 180 ? "0" : "1";
  const sweep = endDeg >= startDeg ? "1" : "0";
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${r} ${r} 0 ${largeArc} ${sweep} ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

// Reference dial mapping:
// 0 GHz sits at -135 deg (bottom-left), 5 GHz sits at 0 deg (12 o'clock top).
// Scale numbers 1..5 sit cleanly inside the graduated tick arc; right side (-135..+135)
// finishes the symmetrical 270 deg radial dot/tick halo with a dim upper-right quadrant.
const DIAL_START_DEG = -135;
const DIAL_TOP_DEG = 0;
const DIAL_END_DEG = 135;

export function CpuTachometer({
  clockMhz,
  maxGhz = 5,
  utilizationPct,
  tempC,
}: CpuTachometerProps) {
  const uid = useId().replace(/:/g, "");
  const coreGlowId = `cpu-center-glow-${uid}`;
  const arcHaloId = `cpu-arc-halo-${uid}`;
  const cyanGlowId = `cpu-cyan-glow-${uid}`;

  const cx = 165;
  const cy = 152;
  const animClock = useAnimatedNumber(clockMhz, 750);
  const animUtil = useAnimatedNumber(utilizationPct, 750);
  const animTemp = useAnimatedNumber(tempC, 750);

  const hasClock = animClock !== null && animClock > 0;
  const displayClock = hasClock ? Math.round(animClock) : null;
  const ghz = hasClock ? Math.max(0, Math.min(maxGhz, animClock / 1000)) : 0;
  const needleDeg =
    DIAL_START_DEG + (ghz / maxGhz) * (DIAL_TOP_DEG - DIAL_START_DEG);

  // Outer dotted burst field (4 concentric rings along the 270° span)
  const dotRings = [118, 128, 138, 148];
  const dotAngles: number[] = [];
  for (let deg = DIAL_START_DEG; deg <= DIAL_END_DEG; deg += 4.5) {
    dotAngles.push(deg);
  }

  // Fine inner scale ticks (72 steps across -135°..+135°)
  const minorTicks: number[] = [];
  for (let i = 0; i <= 72; i += 1) {
    minorTicks.push(DIAL_START_DEG + (i / 72) * (DIAL_END_DEG - DIAL_START_DEG));
  }

  const labels = [1, 2, 3, 4, 5];

  // Bottom utilization mini-arc (-40° around bottom center -> +40°)
  const utilStart = 220;
  const utilEnd = 140;
  const utilSpan = 80;
  const utilFloat =
    animUtil === null ? 0 : Math.max(0, Math.min(100, animUtil));
  const utilClamped = Math.round(utilFloat);
  const utilFillEnd = utilStart - (utilFloat / 100) * utilSpan;

  const needleTip = polarToCartesian(cx, cy, 108, needleDeg);
  const needleBase = polarToCartesian(cx, cy, 16, needleDeg);

  return (
    <div className="relative flex w-full flex-col items-center justify-center select-none">
      <svg
        viewBox="0 0 330 284"
        className="h-auto w-full max-w-[520px] overflow-visible"
        role="img"
        aria-label={`CPU telemetry ${clockMhz ?? "--"} MHz, ${utilizationPct !== null ? Math.round(utilizationPct) : "--"}% utilization, ${tempC ?? "--"} degrees Celsius`}
      >
        <defs>
          <radialGradient id={coreGlowId} cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="#00d8ff" stopOpacity="0.32" />
            <stop offset="52%" stopColor="#00b8ff" stopOpacity="0.11" />
            <stop offset="100%" stopColor="#00b8ff" stopOpacity="0" />
          </radialGradient>
          <filter id={arcHaloId} x="-40%" y="-40%" width="180%" height="180%">
            <feGaussianBlur stdDeviation="5.5" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <filter id={cyanGlowId} x="-35%" y="-35%" width="170%" height="170%">
            <feGaussianBlur stdDeviation="2.6" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Ambient radial core glow */}
        <circle cx={cx} cy={cy} r="144" fill={`url(#${coreGlowId})`} />

        {/* Dotted radial halo */}
        {dotRings.map((r, ringIdx) =>
          dotAngles.map((deg) => {
            const pt = polarToCartesian(cx, cy, r, deg);
            const activeSide = deg <= needleDeg;
            const leftScaleSide = deg <= DIAL_TOP_DEG;
            const opacity = activeSide
              ? 0.56 - ringIdx * 0.11
              : leftScaleSide
                ? 0.34 - ringIdx * 0.06
                : 0.18 - ringIdx * 0.035;
            return (
              <circle
                key={`${r}-${deg}`}
                cx={pt.x}
                cy={pt.y}
                r={ringIdx === 0 ? 1.15 : 1.0}
                fill={activeSide ? "#00d8ff" : leftScaleSide ? "#3a8fa8" : "#4b6273"}
                opacity={Math.max(0.05, opacity)}
              />
            );
          })
        )}

        {/* Inner concentric cyan rings */}
        <circle
          cx={cx}
          cy={cy}
          r="56"
          fill="none"
          stroke="#00d8ff"
          strokeOpacity="0.28"
          strokeWidth="1.25"
        />
        <circle
          cx={cx}
          cy={cy}
          r="43"
          fill="none"
          stroke="#00b8ff"
          strokeOpacity="0.34"
          strokeWidth="10"
        />
        <circle
          cx={cx}
          cy={cy}
          r="16"
          fill="#08131a"
          stroke="#00d8ff"
          strokeOpacity="0.78"
          strokeWidth="1.6"
        />

        {/* Soft cyan outer corona along active scale region */}
        <path
          d={describeArc(cx, cy, 108, DIAL_START_DEG, needleDeg + 8)}
          fill="none"
          stroke="#00d8ff"
          strokeOpacity="0.42"
          strokeWidth="6"
          filter={`url(#${arcHaloId})`}
        />

        {/* Dim right-side continuation arc + bright left scale arc */}
        <path
          d={describeArc(cx, cy, 106, DIAL_TOP_DEG, DIAL_END_DEG)}
          fill="none"
          stroke="#2d3748"
          strokeWidth="1.5"
        />
        <path
          d={describeArc(cx, cy, 106, DIAL_START_DEG, DIAL_TOP_DEG)}
          fill="none"
          stroke="#e8eff7"
          strokeWidth="2.4"
        />
        <path
          d={describeArc(cx, cy, 106, DIAL_START_DEG, needleDeg)}
          fill="none"
          stroke="#00d8ff"
          strokeWidth="2.8"
          filter={`url(#${cyanGlowId})`}
        />

        {/* Graduated inward ticks */}
        {minorTicks.map((deg, idx) => {
          const isLeftActive = deg <= DIAL_TOP_DEG;
          const isMajor = idx % 6 === 0 && isLeftActive;
          const innerR = isMajor ? 89 : 95;
          const outerR = 104;
          const p1 = polarToCartesian(cx, cy, innerR, deg);
          const p2 = polarToCartesian(cx, cy, outerR, deg);
          return (
            <line
              key={deg}
              x1={p1.x}
              y1={p1.y}
              x2={p2.x}
              y2={p2.y}
              stroke={
                isMajor
                  ? "#f1f5f9"
                  : isLeftActive
                    ? "#a9bed4"
                    : "#3c4757"
              }
              strokeWidth={isMajor ? 1.9 : 1}
              opacity={isLeftActive ? 0.92 : 0.65}
            />
          );
        })}

        {/* Bold 12 o'clock top stop tick for 5 GHz ceiling */}
        <line
          x1={cx}
          y1={cy - 107}
          x2={cx}
          y2={cy - 86}
          stroke="#f8fafc"
          strokeWidth="2.4"
        />

        {/* Cyan accent tick on right dim quadrant matching reference */}
        {(() => {
          const p1 = polarToCartesian(cx, cy, 99, 87);
          const p2 = polarToCartesian(cx, cy, 110, 87);
          return (
            <line
              x1={p1.x}
              y1={p1.y}
              x2={p2.x}
              y2={p2.y}
              stroke="#00d8ff"
              strokeWidth="2.8"
              filter={`url(#${cyanGlowId})`}
            />
          );
        })()}

        {/* Numeric GHz scale labels (1..5) positioned inside tick arc matching reference */}
        {labels.map((val) => {
          const deg =
            val === 5
              ? -8
              : DIAL_START_DEG + (val / maxGhz) * (DIAL_TOP_DEG - DIAL_START_DEG);
          const pos = polarToCartesian(cx, cy, 78, deg);
          return (
            <text
              key={val}
              x={pos.x}
              y={pos.y}
              fill="#e2e8f0"
              fontSize="11.5"
              fontWeight="600"
              textAnchor="middle"
              dominantBaseline="central"
            >
              {val}
            </text>
          );
        })}

        {/* Glowing cyan speedometer needle */}
        <line
          x1={needleBase.x}
          y1={needleBase.y}
          x2={needleTip.x}
          y2={needleTip.y}
          stroke="#00d8ff"
          strokeWidth="2.8"
          strokeLinecap="round"
          filter={`url(#${cyanGlowId})`}
        />
        <circle cx={cx} cy={cy} r="4.5" fill="#00d8ff" opacity="0.9" />

        {/* Angled reference wing frame & callout underlines with centered "CPU" label */}
        <polyline
          points="10,254 86,254 128,210 144,210"
          fill="none"
          stroke="#cbd5e1"
          strokeWidth="1.25"
          strokeOpacity="0.88"
          strokeLinejoin="round"
        />
        <polyline
          points="186,210 202,210 244,254 318,254"
          fill="none"
          stroke="#cbd5e1"
          strokeWidth="1.25"
          strokeOpacity="0.88"
          strokeLinejoin="round"
        />
        <text
          x={cx}
          y="210"
          fill="#e2e8f0"
          fontSize="13.5"
          fontWeight="500"
          letterSpacing="1.6"
          textAnchor="middle"
          dominantBaseline="central"
        >
          CPU
        </text>

        {/* Bottom Utilization curved progress arc */}
        <path
          d={describeArc(cx, cy, 78, utilEnd, utilStart)}
          fill="none"
          stroke="#1b2633"
          strokeWidth="4.8"
          strokeLinecap="round"
        />
        {utilFloat > 0.2 && (
          <path
            d={describeArc(cx, cy, 78, utilFillEnd, utilStart)}
            fill="none"
            stroke="#00d8ff"
            strokeWidth="4.8"
            strokeLinecap="round"
            filter={`url(#${cyanGlowId})`}
          />
        )}
        <text
          x={cx}
          y="249"
          fill="#f1f5f9"
          fontSize="14.5"
          fontWeight="500"
          textAnchor="middle"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          {animUtil === null ? "--" : `${utilClamped}%`}
        </text>
        <text
          x={cx}
          y="266"
          fill="#94a3b8"
          fontSize="11"
          textAnchor="middle"
        >
          Utilization
        </text>

        {/* Bottom-left Clock callout */}
        <text
          x="61"
          y="246"
          fill="#f1f5f9"
          fontSize="26"
          fontWeight="300"
          textAnchor="end"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          {displayClock !== null ? displayClock : "--"}
        </text>
        <text
          x="64"
          y="246"
          fill="#cbd5e1"
          fontSize="10.5"
          textAnchor="start"
        >
          MHz
        </text>
        <g transform="translate(29, 263)" stroke="#9ca3af" fill="none">
          <path d="M2 8a6 6 0 0 1 12 0" strokeWidth="1.2" />
          <path d="M8 8l3-3" strokeWidth="1.2" strokeLinecap="round" />
        </g>
        <text
          x="61"
          y="271"
          fill="#9ca3af"
          fontSize="10.5"
          textAnchor="middle"
        >
          Clock
        </text>

        {/* Bottom-right Temperature callout */}
        <text
          x="291"
          y="246"
          fill="#f1f5f9"
          fontSize="26"
          fontWeight="300"
          textAnchor="end"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          {animTemp === null ? "--" : Math.round(animTemp)}
        </text>
        <text
          x="295"
          y="236"
          fill="#cbd5e1"
          fontSize="11"
          textAnchor="start"
        >
          °C
        </text>
        <g transform="translate(244, 261)" stroke="#9ca3af" fill="none">
          <path d="M5 2v7.2a2 2 0 1 0 2.5 0V2a1.25 1.25 0 0 0-2.5 0Z" strokeWidth="1.15" />
        </g>
        <text
          x="286"
          y="271"
          fill="#9ca3af"
          fontSize="10.5"
          textAnchor="middle"
        >
          Temperature
        </text>
      </svg>
    </div>
  );
}
