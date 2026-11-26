import { useId } from "react";
import { useAnimatedNumber } from "../../hooks/useAnimatedNumber";

interface MiniRadialGaugeProps {
  percent: number | null;
  title: string;
  subtitle: string;
  numericValue?: { value: number | null; unit: string };
  detailTitle?: string;
}

function polar(cx: number, cy: number, r: number, angleDeg: number) {
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
  const start = polar(cx, cy, r, startDeg);
  const end = polar(cx, cy, r, endDeg);
  const largeArc = Math.abs(endDeg - startDeg) <= 180 ? "0" : "1";
  const sweep = endDeg >= startDeg ? "1" : "0";
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${r} ${r} 0 ${largeArc} ${sweep} ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

// Reference mini-dials orient 0% at bottom (6 o'clock) clockwise, with a prominent
// olive-lime ring track, glowing lime active needle, white radial endpoint tick,
// and prominent percentage number centered above "%".
export function MiniRadialGauge({
  percent,
  title,
  subtitle,
  numericValue,
  detailTitle,
}: MiniRadialGaugeProps) {
  const filterId = `mini-gauge-${useId().replace(/:/g, "")}`;
  const isAvailable = percent !== null && Number.isFinite(percent);
  const animPercent = useAnimatedNumber(isAvailable ? percent : null, 750);
  const animNumeric = useAnimatedNumber(numericValue?.value ?? null, 750);

  const clampedFloat =
    animPercent !== null && Number.isFinite(animPercent)
      ? Math.max(0, Math.min(100, animPercent))
      : 0;
  const clamped = Math.round(clampedFloat);
  const displaySubtitle =
    numericValue && animNumeric !== null && Number.isFinite(animNumeric)
      ? `${Math.round(animNumeric)} ${numericValue.unit}`
      : subtitle;

  const cx = 36;
  const cy = 36;
  const startDeg = 180;
  const angleDeg = startDeg + (clampedFloat / 100) * 320;
  const arcRadius = 26.5;
  const needleInner = polar(cx, cy, 19, angleDeg);
  const needleOuter = polar(cx, cy, 34.5, angleDeg);
  const tickInner = polar(cx, cy, 24, angleDeg);
  const tickOuter = polar(cx, cy, 34.5, angleDeg);

  return (
    <div
      className="flex items-center gap-1.5 select-none"
      title={detailTitle ?? `${title}: ${displaySubtitle}`}
    >
      <svg
        viewBox="0 0 72 72"
        className="h-[75px] w-[75px] shrink-0"
        role="img"
        aria-label={
          isAvailable
            ? `${title}: ${clamped}% (${subtitle})`
            : `${title}: unavailable`
        }
      >
        <defs>
          <filter
            id={filterId}
            x="-45%"
            y="-45%"
            width="190%"
            height="190%"
          >
            <feGaussianBlur stdDeviation="2.4" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Outer boundary circle (~25% larger ring diameter) */}
        <circle
          cx={cx}
          cy={cy}
          r="34"
          fill={isAvailable ? "#0d1409" : "#11141a"}
          fillOpacity={isAvailable ? "0.72" : "0.65"}
          stroke={isAvailable ? "#4b6816" : "#374151"}
          strokeOpacity="0.85"
          strokeWidth="1.25"
        />

        {/* Thick background track ring */}
        <circle
          cx={cx}
          cy={cy}
          r={arcRadius}
          fill="none"
          stroke={isAvailable ? "#21320e" : "#1f242d"}
          strokeWidth="11"
        />

        {/* Subtle olive progress arc sweep from bottom clockwise */}
        {isAvailable && clampedFloat > 0.2 && (
          <path
            d={describeArc(cx, cy, arcRadius, startDeg, angleDeg)}
            fill="none"
            stroke="#5a8216"
            strokeOpacity="0.65"
            strokeWidth="11"
          />
        )}

        {/* Inner concentric ring separating numeric readout */}
        <circle
          cx={cx}
          cy={cy}
          r="19.5"
          fill="#0a0e0d"
          stroke={isAvailable ? "#486318" : "#374151"}
          strokeOpacity="0.85"
          strokeWidth="1.15"
        />

        {isAvailable && (
          <>
            {/* Glowing lime active radial indicator */}
            <line
              x1={needleInner.x}
              y1={needleInner.y}
              x2={needleOuter.x}
              y2={needleOuter.y}
              stroke="#9be000"
              strokeWidth="4.4"
              strokeLinecap="butt"
              filter={`url(#${filterId})`}
            />

            {/* Crisp white radial endpoint tick on active arc edge */}
            <line
              x1={tickInner.x}
              y1={tickInner.y}
              x2={tickOuter.x}
              y2={tickOuter.y}
              stroke="#ffffff"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </>
        )}

        {/* Prominent centered percentage value (~19px scaled) */}
        <text
          x={cx}
          y={cy - 2.5}
          fill={isAvailable ? "#f1f5f9" : "#6b7280"}
          fontSize="18.5"
          fontWeight="500"
          textAnchor="middle"
          dominantBaseline="central"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          {isAvailable ? clamped : "--"}
        </text>

        {/* Compact % sign neatly positioned underneath */}
        <text
          x={cx}
          y={cy + 12.5}
          fill={isAvailable ? "#cbd5e1" : "#6b7280"}
          fontSize="8.5"
          fontWeight="500"
          textAnchor="middle"
        >
          %
        </text>
      </svg>

      <div className="min-w-0 leading-snug">
        <div
          className={`truncate text-[13px] font-normal ${
            isAvailable ? "text-[#d1d5db]" : "text-[#6b7280]"
          }`}
        >
          {title}
        </div>
        <div
          className={`mt-0.5 truncate text-[13px] font-normal tracking-tight tabular-nums ${
            isAvailable ? "text-slate-100" : "text-[#6b7280]"
          }`}
        >
          {displaySubtitle}
        </div>
      </div>
    </div>
  );
}
