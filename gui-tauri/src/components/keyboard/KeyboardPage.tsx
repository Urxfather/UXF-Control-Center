import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { rgbToHex, useKeyboardBackend } from "../../hooks/useKeyboardBackend";
import { ColorBacklightWheel } from "./ColorBacklightWheel";
import { KeyboardPreview } from "./KeyboardPreview";

function splitSeconds(total: number): { hr: number; min: number; sec: number } {
  const safe = Math.max(1, Math.min(86_400, Math.round(total)));
  const hr = Math.floor(safe / 3600);
  const min = Math.floor((safe % 3600) / 60);
  const sec = safe % 60;
  return { hr, min, sec };
}

function BracketedDigitBox({
  label,
  value,
  max,
  disabled,
  onChange,
  onCommit,
}: {
  label: string;
  value: number;
  max: number;
  disabled: boolean;
  onChange: (next: number) => void;
  onCommit: () => void;
}) {
  return (
    <div className="flex flex-col items-start gap-1">
      <span className="text-[12px] font-medium text-[#9ca3af]">{label}</span>
      <div className="relative flex h-[44px] w-[44px] items-center justify-center bg-[#0d1118]">
        {/* Reference corner brackets */}
        <span aria-hidden="true" className="pointer-events-none absolute top-0 left-0 h-2 w-2 border-t border-l border-[#9ca3af]/80" />
        <span aria-hidden="true" className="pointer-events-none absolute top-0 right-0 h-2 w-2 border-t border-r border-[#9ca3af]/80" />
        <span aria-hidden="true" className="pointer-events-none absolute bottom-0 left-0 h-2 w-2 border-b border-l border-[#9ca3af]/80" />
        <span aria-hidden="true" className="pointer-events-none absolute right-0 bottom-0 h-2 w-2 border-r border-b border-[#9ca3af]/80" />
        <input
          type="number"
          min={0}
          max={max}
          disabled={disabled}
          value={value}
          aria-label={label}
          onChange={(e) => {
            const parsed = Number.parseInt(e.target.value || "0", 10);
            onChange(Number.isNaN(parsed) ? 0 : Math.max(0, Math.min(max, parsed)));
          }}
          onBlur={onCommit}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.currentTarget.blur();
            }
          }}
          className="w-full select-text bg-transparent text-center font-mono text-[16px] font-semibold text-white tabular-nums focus:outline-none [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
        />
      </div>
    </div>
  );
}

function StepBrightnessSlider({
  step,
  disabled,
  onChange,
}: {
  step: number;
  disabled: boolean;
  onChange: (nextStep: number) => void;
}) {
  const trackRef = useRef<HTMLDivElement | null>(null);
  const draggingRef = useRef(false);

  const clamped = Math.max(0, Math.min(4, Math.round(step)));
  const pct = (clamped / 4) * 100;

  const stepFromPointer = (clientX: number): number => {
    const el = trackRef.current;
    if (!el) {
      return clamped;
    }
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) {
      return clamped;
    }
    const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
    return Math.round(ratio * 4);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || e.button !== 0) {
      return;
    }
    e.preventDefault();
    draggingRef.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    onChange(stepFromPointer(e.clientX));
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || !draggingRef.current) {
      return;
    }
    const next = stepFromPointer(e.clientX);
    if (next !== clamped) {
      onChange(next);
    }
  };

  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (disabled || !draggingRef.current) {
      return;
    }
    draggingRef.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
    onChange(stepFromPointer(e.clientX));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) {
      return;
    }
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") {
      next = Math.min(4, clamped + 1);
    } else if (e.key === "ArrowLeft" || e.key === "ArrowDown") {
      next = Math.max(0, clamped - 1);
    } else if (e.key === "Home") {
      next = 0;
    } else if (e.key === "End") {
      next = 4;
    }
    if (next !== null) {
      e.preventDefault();
      onChange(next);
    }
  };

  return (
    <div className="flex w-[220px] flex-col select-none">
      <div className="mb-2.5 text-center text-[14.5px] font-medium text-[#cbd5e1]">
        Brightness
      </div>

      <div className="mb-1 flex items-center justify-between text-[12px] font-medium tabular-nums text-[#e5e7eb]">
        <span>OFF</span>
        <span className="text-[11px] tabular-nums text-[#6b7280]">{clamped === 0 ? "Off" : `Level ${clamped}`}</span>
        <span>4</span>
      </div>

      <div
        ref={trackRef}
        role="slider"
        tabIndex={disabled ? -1 : 0}
        aria-label="Keyboard brightness"
        aria-disabled={disabled}
        aria-valuemin={0}
        aria-valuemax={4}
        aria-valuenow={clamped}
        aria-valuetext={clamped === 0 ? "OFF" : `Level ${clamped}`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onKeyDown={onKeyDown}
        className={`relative py-2.5 touch-none focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
          disabled ? "cursor-not-allowed opacity-50" : "cursor-ew-resize"
        }`}
      >
        {/* Base thin bar */}
        <div className="relative h-[2px] w-full bg-[#242c39]">
          <div
            className="h-full bg-[#9be000] transition-[width] duration-150"
            style={{ width: `${pct}%` }}
          />
        </div>

        {/* 5 discrete dot stops (OFF, 1, 2, 3, 4) */}
        {[0, 1, 2, 3, 4].map((stop) => (
          <span
            key={stop}
            aria-hidden="true"
            style={{ left: `${(stop / 4) * 100}%` }}
            className={`absolute top-1.5 h-1 w-1 -translate-x-1/2 rounded-full ${
              stop <= clamped ? "bg-[#cbd5e1]" : "bg-[#4b5563]"
            }`}
          />
        ))}

        {/* Reference upward marker under active stop */}
        <span
          aria-hidden="true"
          style={{
            left: `clamp(0px, calc(${pct}% - 4px), calc(100% - 8px))`,
          }}
          className="absolute bottom-0 h-0 w-0 border-x-[4px] border-b-[6px] border-x-transparent border-b-[#e5e7eb] transition-[left] duration-150"
        />
      </div>
    </div>
  );
}

export function KeyboardPage() {
  const {
    keyboard,
    keyboardError,
    activeColor,
    activeColorHex,
    activeStep,
    setColor,
    setBrightnessStep,
    setEffect,
    setSleepTimer,
  } = useKeyboardBackend();

  const [timerDraft, setTimerDraft] = useState<{ hr: number; min: number; sec: number }>(() =>
    splitSeconds(keyboard?.sleepTimerSeconds ?? 5)
  );

  useEffect(() => {
    if (keyboard?.sleepTimerSeconds) {
      setTimerDraft(splitSeconds(keyboard.sleepTimerSeconds));
    }
  }, [keyboard?.sleepTimerSeconds]);

  const daemonConnected = keyboard?.daemonConnected ?? false;
  const available = keyboard?.available ?? false;
  const sleepEnabled = keyboard?.sleepTimerEnabled ?? false;
  const idleSupported = keyboard?.idleMonitorSupported ?? true;
  const currentEffect = keyboard?.effect ?? "static";
  const rainbowSupported = keyboard?.rainbowSupported ?? false;
  const bootSupported = keyboard?.bootEffectSupported ?? false;

  const commitSleepTimer = (nextEnabled: boolean, draft = timerDraft) => {
    const total = Math.max(1, draft.hr * 3600 + draft.min * 60 + draft.sec);
    void setSleepTimer(nextEnabled, total);
  };

  return (
    <section
      aria-labelledby="led-keyboard-heading"
      className="flex h-full flex-col justify-between gap-2 overflow-y-auto bg-[#06090e] px-6 py-3 select-none"
    >
      {/* Top Row: Reference Title + Real Hardware Mode Selector */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2
            id="led-keyboard-heading"
            className="text-[16px] font-normal tracking-[0.02em] text-[#e5e7eb]"
          >
            LED Keyboard
          </h2>
          <span
            className="rounded border border-[#1a202c] bg-[#0a0e15] px-2 py-0.5 text-[10px] font-medium text-[#7c8594]"
            title={
              daemonConnected
                ? `Connected to acer-control-daemon v${keyboard?.daemonVersion ?? "?"} (rgb:kbd_backlight)`
                : "Acer daemon disconnected"
            }
          >
            {keyboard?.zonesLabel ?? "Single-Zone RGB"}
          </span>
        </div>

        {/* Lighting Effect selector (only real hardware features active; Rainbow explicitly disabled) */}
        <div className="flex items-center gap-1.5">
          <button
            type="button"
            disabled={!available}
            onClick={() => void setEffect("static")}
            className={`rounded border px-2.5 py-1 text-[11px] font-medium transition-all duration-100 active:scale-[0.98] ${
              currentEffect === "static"
                ? "border-[#9be000]/60 bg-[#9be000]/15 text-[#9be000]"
                : "border-[#1d2330] bg-[#0a0e14] text-[#9ca3af] hover:border-[#2c3547] hover:text-slate-200"
            } ${!available ? "cursor-not-allowed opacity-50" : "cursor-pointer"}`}
          >
            Static
          </button>

          <button
            type="button"
            disabled={!available || !keyboard?.breathingSupported}
            onClick={() => void setEffect("breathing")}
            className={`rounded border px-2.5 py-1 text-[11px] font-medium transition-all duration-100 active:scale-[0.98] ${
              currentEffect === "breathing"
                ? "border-[#9be000]/60 bg-[#9be000]/15 text-[#9be000]"
                : "border-[#1d2330] bg-[#0a0e14] text-[#9ca3af] hover:border-[#2c3547] hover:text-slate-200"
            } ${
              !available || !keyboard?.breathingSupported
                ? "cursor-not-allowed opacity-50"
                : "cursor-pointer"
            }`}
          >
            Breathing
          </button>

          <button
            type="button"
            disabled={!available || !rainbowSupported}
            aria-disabled={!available || !rainbowSupported}
            onClick={() => {
              if (rainbowSupported) {
                void setEffect("rainbow");
              }
            }}
            title={
              rainbowSupported
                ? "Cycle spectrum colors across keyboard backlight (D-Bus)"
                : (keyboard?.rainbowReason ??
                  "Rainbow requires StartRainbow/GetRainbowSelected on root acer-control-daemon; 1-zone RGB hardware has no firmware EC rainbow mode")
            }
            className={`rounded border px-2.5 py-1 text-[11px] font-medium transition-all duration-100 active:scale-[0.98] ${
              !available || !rainbowSupported
                ? "cursor-not-allowed border-[#171c26] bg-[#080b10] text-[#525b6a] opacity-65"
                : currentEffect === "rainbow"
                  ? "cursor-pointer border-[#9be000]/60 bg-[#9be000]/15 text-[#9be000]"
                  : "cursor-pointer border-[#1d2330] bg-[#0a0e14] text-[#9ca3af] hover:border-[#2c3547] hover:text-slate-200"
            }`}
          >
            Rainbow
          </button>
        </div>
      </div>

      {/* Inline error notice if daemon unavailable */}
      {keyboardError && (
        <div className="mt-2 rounded border border-amber-500/30 bg-amber-500/10 px-3 py-1.5 text-[11px] text-amber-300">
          {keyboardError}
        </div>
      )}

      {/* Middle 3-Column Control Row: Sleep Timer | Color Backlight Wheel | Brightness 0..4 */}
      <div className="grid grid-cols-1 items-center gap-4 py-1 lg:grid-cols-[minmax(240px,1fr)_auto_minmax(220px,1fr)]">
        {/* Left: Keyboard Sleep Timer (backed by Mutter.IdleMonitor + transient fade) */}
        <div className="flex flex-col items-start gap-2.5 justify-self-center lg:justify-self-start">
          <div className="flex flex-wrap items-center gap-3.5">
            <div className="flex items-center gap-2 text-[14.5px] font-medium text-[#e5e7eb]">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.75"
                className="h-5 w-5 text-[#cbd5e1]"
                aria-hidden="true"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M3 12a9 9 0 1 0 3-6.7L3 8m0-4v4h4m5-1v5l3 2"
                />
              </svg>
              <span>Keyboard Sleep Timer</span>
            </div>

            <div className="flex items-center gap-3 text-[13px]">
              <button
                type="button"
                disabled={!idleSupported}
                onClick={() => commitSleepTimer(false)}
                className={`flex items-center gap-1.5 transition-transform duration-100 active:scale-[0.98] ${
                  idleSupported ? "cursor-pointer" : "cursor-not-allowed opacity-50"
                }`}
              >
                <span
                  className={`flex h-4 w-4 items-center justify-center border ${
                    !sleepEnabled
                      ? "border-[#9be000] bg-[#9be000]/20 text-[#9be000]"
                      : "border-[#394254] bg-[#0a0d13]"
                  }`}
                >
                  {!sleepEnabled && <span className="h-2 w-2 bg-[#9be000]" />}
                </span>
                <span className={!sleepEnabled ? "text-[#e5e7eb]" : "text-[#7c8594]"}>
                  Disable
                </span>
              </button>

              <button
                type="button"
                disabled={!idleSupported}
                onClick={() => commitSleepTimer(true)}
                className={`flex items-center gap-1.5 transition-transform duration-100 active:scale-[0.98] ${
                  idleSupported ? "cursor-pointer" : "cursor-not-allowed opacity-50"
                }`}
              >
                <span
                  className={`flex h-4 w-4 items-center justify-center border ${
                    sleepEnabled
                      ? "border-[#9be000] bg-[#9be000]/20 text-[#9be000]"
                      : "border-[#394254] bg-[#0a0d13]"
                  }`}
                >
                  {sleepEnabled && <span className="h-2 w-2 bg-[#9be000]" />}
                </span>
                <span className={sleepEnabled ? "text-[#9be000]" : "text-[#7c8594]"}>
                  Enable
                </span>
              </button>
            </div>
          </div>

          <div className="flex items-end gap-2.5 pl-1">
            <BracketedDigitBox
              label="Hr"
              value={timerDraft.hr}
              max={23}
              disabled={!idleSupported}
              onChange={(hr) => setTimerDraft((d) => ({ ...d, hr }))}
              onCommit={() => {
                if (sleepEnabled) {
                  commitSleepTimer(true);
                }
              }}
            />
            <span className="pb-2.5 font-mono text-base text-[#9ca3af]">:</span>
            <BracketedDigitBox
              label="Min"
              value={timerDraft.min}
              max={59}
              disabled={!idleSupported}
              onChange={(min) => setTimerDraft((d) => ({ ...d, min }))}
              onCommit={() => {
                if (sleepEnabled) {
                  commitSleepTimer(true);
                }
              }}
            />
            <span className="pb-2.5 font-mono text-base text-[#9ca3af]">:</span>
            <BracketedDigitBox
              label="Sec"
              value={timerDraft.sec}
              max={59}
              disabled={!idleSupported}
              onChange={(sec) => setTimerDraft((d) => ({ ...d, sec }))}
              onCommit={() => {
                if (sleepEnabled) {
                  commitSleepTimer(true);
                }
              }}
            />
          </div>

          <span className="pl-1 text-[11px] tabular-nums text-[#6b7280]">
            {keyboard?.idleStatus ?? "Idle dimming off"}
          </span>
        </div>

        {/* Center: Dynamic Color Backlight Wheel */}
        <div className="justify-self-center">
          <ColorBacklightWheel
            color={activeColor}
            colorHex={activeColorHex || rgbToHex([255, 0, 0])}
            brightnessStep={activeStep}
            breathing={currentEffect === "breathing"}
            dimmed={keyboard?.idleDimmed ?? false}
            disabled={!available}
            onSelectColor={(rgb, commit) => void setColor(rgb, commit)}
          />
        </div>

        {/* Right: 5-Stop Brightness Slider (OFF, 1, 2, 3, 4) */}
        <div className="flex w-full justify-center lg:justify-end">
          <StepBrightnessSlider
            step={activeStep}
            disabled={!available}
            onChange={(nextStep) => void setBrightnessStep(nextStep)}
          />
        </div>
      </div>

      {/* Bottom Section: Full QWERTY + Numpad Visual Keyboard + Disabled Boot Lighting Checkbox */}
      <div className="flex flex-col gap-1.5">
        <KeyboardPreview
          color={activeColor}
          brightnessStep={activeStep}
          breathing={currentEffect === "breathing"}
          dimmed={keyboard?.idleDimmed ?? false}
        />

        <div className="mx-auto flex w-full max-w-[620px] justify-end">
          <button
            type="button"
            disabled={!bootSupported}
            aria-disabled={!bootSupported}
            title={
              keyboard?.bootEffectReason ??
              "Pre-boot lighting override is not supported on this platform"
            }
            className="flex cursor-not-allowed items-center gap-2 text-[11px] text-[#525b6a] opacity-75"
          >
            <span className="h-3.5 w-3.5 border border-[#2a313e] bg-[#080b10]" />
            <span>Boot Lighting Effect</span>
            <span className="font-mono text-[9.5px] text-[#475569]">(Unsupported)</span>
          </button>
        </div>
      </div>
    </section>
  );
}
