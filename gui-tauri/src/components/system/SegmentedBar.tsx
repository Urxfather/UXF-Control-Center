import {
  useRef,
  type KeyboardEvent,
  type PointerEvent,
  type ReactNode,
} from "react";

interface SegmentedBarProps {
  label: string;
  valueText: string;
  percent: number;
  segments?: number;
  disabled?: boolean;
}

export function SegmentedBar({
  label,
  valueText,
  percent,
  segments = 42,
  disabled = false,
}: SegmentedBarProps) {
  const clamped = Math.max(0, Math.min(100, percent));
  const activeCount = disabled ? 0 : Math.round((clamped / 100) * segments);

  return (
    <div className={`space-y-1.5 ${disabled ? "opacity-60" : ""}`}>
      <div className="flex w-full items-center justify-between text-[13.5px]">
        <span className={disabled ? "text-[#7c8594]" : "text-[#cbd5e1]"}>
          {label}
        </span>
        <span
          className={`tabular-nums ${
            disabled ? "text-[#7c8594]" : "text-slate-100"
          }`}
        >
          {valueText}
        </span>
      </div>
      <div
        className={`flex h-2.5 w-full items-center gap-[1.5px] rounded-none border bg-[#070a0f] px-0.5 py-0.5 ${
          disabled ? "border-[#1f2630]" : "border-[#262b36]"
        }`}
        aria-label={`${label}: ${valueText}`}
      >
        {Array.from({ length: segments }, (_, i) => {
          const lit = i < activeCount;
          return (
            <span
              key={i}
              className={`h-full flex-1 ${
                lit
                  ? "bg-[#9be000] shadow-[0_0_4px_rgba(155,224,0,0.45)]"
                  : "bg-[#1a2029]"
              }`}
            />
          );
        })}
      </div>
    </div>
  );
}

interface ConfigSliderRowProps {
  icon: ReactNode;
  label: string;
  valueText?: string;
  minLabel: string;
  maxLabel: string;
  percent: number;
  segments?: number;
  disabled?: boolean;
  mutedTrack?: boolean;
  trackVariant?: "active" | "muted" | "standby";
  stepPercent?: number;
  ariaMin?: number;
  ariaMax?: number;
  ariaValueNow?: number;
  ariaValueText?: string;
  tooltip?: string;
  onChange?: (nextPercent: number, commit: boolean) => void;
}

export function ConfigSliderRow({
  icon,
  label,
  valueText,
  minLabel,
  maxLabel,
  percent,
  segments = 38,
  disabled = true,
  mutedTrack = false,
  trackVariant,
  stepPercent = 2,
  ariaMin = 0,
  ariaMax = 100,
  ariaValueNow,
  ariaValueText,
  tooltip,
  onChange,
}: ConfigSliderRowProps) {
  const tone = trackVariant ?? (mutedTrack ? "muted" : "active");
  const trackRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef(false);

  const clamped = Math.max(0, Math.min(100, percent));
  const activeCount = disabled ? 0 : Math.round((clamped / 100) * segments);
  const isInteractive = !disabled && Boolean(onChange);

  const percentFromClientX = (clientX: number): number => {
    const el = trackRef.current;
    if (!el) {
      return clamped;
    }
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) {
      return clamped;
    }
    return Math.max(
      0,
      Math.min(100, ((clientX - rect.left) / rect.width) * 100)
    );
  };

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (!isInteractive || event.button !== 0) {
      return;
    }
    event.preventDefault();
    draggingRef.current = true;
    event.currentTarget.setPointerCapture(event.pointerId);
    onChange?.(percentFromClientX(event.clientX), false);
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    if (!isInteractive || !draggingRef.current) {
      return;
    }
    onChange?.(percentFromClientX(event.clientX), false);
  };

  const handlePointerUp = (event: PointerEvent<HTMLDivElement>) => {
    if (!isInteractive || !draggingRef.current) {
      return;
    }
    draggingRef.current = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    onChange?.(percentFromClientX(event.clientX), true);
  };

  const handlePointerCancel = (event: PointerEvent<HTMLDivElement>) => {
    if (!draggingRef.current) {
      return;
    }
    draggingRef.current = false;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    onChange?.(clamped, true);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!isInteractive) {
      return;
    }
    let next: number | null = null;
    switch (event.key) {
      case "ArrowRight":
      case "ArrowUp":
        next = Math.min(100, clamped + stepPercent);
        break;
      case "ArrowLeft":
      case "ArrowDown":
        next = Math.max(0, clamped - stepPercent);
        break;
      case "PageUp":
        next = Math.min(100, clamped + stepPercent * 5);
        break;
      case "PageDown":
        next = Math.max(0, clamped - stepPercent * 5);
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = 100;
        break;
    }
    if (next !== null) {
      event.preventDefault();
      onChange?.(next, true);
    }
  };

  return (
    <div
      className={`select-none ${
        disabled ? "pointer-events-none cursor-not-allowed opacity-65" : ""
      }`}
      title={tooltip ?? (disabled ? `${label} — Unavailable` : label)}
    >
      <div
        className={`flex items-center gap-1.5 text-[13.5px] ${
          disabled ? "text-[#848d9b]" : "text-[#e5e7eb]"
        }`}
      >
        <span className={disabled ? "text-[#848d9b]" : "text-[#cbd5e1]"}>
          {icon}
        </span>
        <span>{label}</span>
        {valueText && (
          <span
            className={`ml-auto truncate text-[11px] font-normal tabular-nums ${
              disabled
                ? "text-[#7c8594]"
                : tone === "muted"
                  ? "text-[#9ca3af]"
                  : tone === "standby"
                    ? "text-amber-300/90"
                    : "text-[#cbd5e1]"
            }`}
          >
            {valueText}
          </span>
        )}
      </div>

      <div
        className={`mt-1 flex items-center justify-between text-[11px] font-medium tabular-nums ${
          disabled ? "text-[#9ca3af]" : "text-[#e5e7eb]"
        }`}
      >
        <span>{minLabel}</span>
        <span>{maxLabel}</span>
      </div>

      <div
        ref={trackRef}
        role="slider"
        tabIndex={isInteractive ? 0 : -1}
        aria-label={label}
        aria-disabled={disabled}
        aria-valuemin={ariaMin}
        aria-valuemax={ariaMax}
        aria-valuenow={
          disabled ? ariaMin : Math.round(ariaValueNow ?? clamped)
        }
        aria-valuetext={ariaValueText ?? valueText}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerCancel}
        onKeyDown={handleKeyDown}
        className={`relative mt-0.5 pb-[7px] touch-none focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
          isInteractive ? "cursor-ew-resize" : "cursor-not-allowed"
        }`}
      >
        <div
          className={`flex h-[10px] items-center gap-[1.5px] rounded-none border bg-[#070a0f] px-0.5 py-[1px] ${
            disabled ? "border-[#222a35]" : "border-[#262b36]"
          }`}
        >
          {Array.from({ length: segments }, (_, i) => {
            const lit = i < activeCount;
            return (
              <span
                key={i}
                className={`h-[6px] flex-1 ${
                  disabled
                    ? "bg-[#2b3340]"
                    : lit
                      ? tone === "muted"
                        ? "bg-[#6b7280]/75"
                        : tone === "standby"
                          ? "bg-amber-400/65"
                          : "bg-[#9be000]/90"
                      : "bg-[#1a2029]"
                }`}
              />
            );
          })}
        </div>

        {/* Upward-pointing triangular slider marker visible in reference Configuration panel */}
        <span
          aria-hidden="true"
          style={{
            left: `clamp(0px, calc(${disabled ? 0 : clamped}% - 4px), calc(100% - 8px))`,
          }}
          className={`absolute bottom-0 h-0 w-0 border-x-[4px] border-b-[6px] border-x-transparent ${
            disabled ? "border-b-[#6b7280]" : "border-b-[#e5e7eb]"
          }`}
        />
      </div>
    </div>
  );
}
