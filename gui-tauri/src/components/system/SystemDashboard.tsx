import type { ReactNode } from "react";
import { useDesktopControls } from "../../hooks/useDesktopControls";
import { usePreferences } from "../../hooks/usePreferences";
import { useSystemBackend } from "../../hooks/useSystemBackend";
import type { DesktopControlsState } from "../../types/system";
import { CpuTachometer } from "./CpuTachometer";
import { MiniRadialGauge } from "./MiniRadialGauge";
import {
  AirplaneIcon,
  BlueLightIcon,
  BrightnessIcon,
  CameraIcon,
  FnLockIcon,
  KeycapBadgeIcon,
  SuperFnSwapIcon,
  SuperLockIcon,
  TouchpadIcon,
  VolumeIcon,
} from "./PreferenceIcons";
import { ConfigSliderRow, SegmentedBar } from "./SegmentedBar";

const PREFERENCE_TILES = [
  { id: "super-lock", fallbackLabel: "Super Lock" },
  { id: "super-fn-swap", fallbackLabel: "Super-Fn Swap" },
  { id: "fn-lock", fallbackLabel: "Fn Lock" },
  { id: "camera", fallbackLabel: "Camera" },
  { id: "touchpad", fallbackLabel: "Touchpad" },
  { id: "airplane", fallbackLabel: "Airplane" },
  { id: "num-lock", fallbackLabel: "Num Lock" },
  { id: "scr-lock", fallbackLabel: "Scroll Lock" },
  { id: "caps-lock", fallbackLabel: "Caps Lock" },
] as const;

type PreferenceTileId = (typeof PREFERENCE_TILES)[number]["id"];

function renderPreferenceIcon(id: PreferenceTileId, active: boolean): ReactNode {
  switch (id) {
    case "super-lock":
      return <SuperLockIcon active={active} />;
    case "super-fn-swap":
      return <SuperFnSwapIcon active={active} />;
    case "fn-lock":
      return <FnLockIcon active={active} />;
    case "camera":
      return <CameraIcon active={active} />;
    case "touchpad":
      return <TouchpadIcon active={active} />;
    case "airplane":
      return <AirplaneIcon active={active} />;
    case "num-lock":
      return <KeycapBadgeIcon text="Num" active={active} />;
    case "scr-lock":
      return <KeycapBadgeIcon text="Scr" active={active} />;
    case "caps-lock":
      return <KeycapBadgeIcon text="Caps" active={active} />;
  }
}

const FAN_MODES = [
  { id: "auto", label: "Auto", supported: true },
  { id: "max", label: "Max", supported: true },
  { id: "custom", label: "Custom", supported: false },
  { id: "iqst", label: "IQST", supported: false },
] as const;

function formatScheduleHour(hour: number) {
  const totalMinutes = Math.max(0, Math.round((hour % 24) * 60));
  const hh = Math.floor(totalMinutes / 60) % 24;
  const mm = totalMinutes % 60;
  return `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`;
}

function describeBlueLightControl(
  blueLight: DesktopControlsState["blueLight"] | undefined,
  liveLevel: number
) {
  const available = Boolean(blueLight?.available);
  const isAtZero = liveLevel <= 0.05;
  const draftKelvin = Math.round(4700 - liveLevel * 300);
  const kelvin =
    blueLight &&
    blueLight.enabled &&
    Math.abs(blueLight.level - liveLevel) < 0.05
      ? blueLight.kelvin
      : draftKelvin;

  const isDisabledState =
    available &&
    ((!blueLight?.enabled && isAtZero) ||
      (blueLight?.statusReason === "disabled" && isAtZero) ||
      isAtZero);
  const isActive = available && !isAtZero && blueLight?.statusReason === "active";

  const valueText = !available
    ? "Unavailable"
    : isDisabledState
      ? "Off"
      : isActive
        ? `${kelvin}K`
        : blueLight?.statusReason === "paused_until_tomorrow"
          ? `Paused · ${kelvin}K`
          : blueLight?.statusReason === "schedule_inactive"
            ? `Scheduled · ${kelvin}K`
            : `${kelvin}K`;

  const scheduleDesc = blueLight?.scheduleAutomatic
    ? "sunset-to-sunrise schedule"
    : blueLight && blueLight.scheduleFrom !== blueLight.scheduleTo
      ? `schedule (${formatScheduleHour(blueLight.scheduleFrom)}–${formatScheduleHour(blueLight.scheduleTo)})`
      : "schedule";

  const tooltip = !available
    ? (blueLight?.error ?? "GNOME Night Light unavailable")
    : isDisabledState
      ? `Blue Light Filter is OFF (neutral colour temperature); move slider above 0 to activate Night Light (preserves your GNOME ${scheduleDesc})`
      : isActive
        ? `GNOME Night Light active at ${kelvin}K (move slider to 0 to turn OFF)`
        : blueLight?.statusReason === "paused_until_tomorrow"
          ? `GNOME Night Light is paused until tomorrow; move slider to resume at ${kelvin}K`
          : `GNOME Night Light enabled at ${kelvin}K; currently outside your GNOME ${scheduleDesc} window (saved for automatic activation)`;

  return {
    available,
    valueText,
    tooltip,
    percent: available ? (isDisabledState ? 0 : (liveLevel / 10) * 100) : 0,
    trackVariant: (isActive ? "active" : "standby") as "active" | "standby",
  };
}

function DashboardCard({
  title,
  headerRight,
  className = "",
  children,
}: {
  title: string;
  headerRight?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={`relative rounded-none border border-[#1f2630] bg-[#0b0e13] px-2.5 py-2 ${className}`}
    >
      {/* Subtle L-shaped top-left and bottom-right corner accents matching reference */}
      <span
        aria-hidden="true"
        className="pointer-events-none absolute -top-px -left-px h-2 w-2 border-t-[1.5px] border-l-[1.5px] border-[#6b7280]"
      />
      <span
        aria-hidden="true"
        className="pointer-events-none absolute -right-px -bottom-px h-2 w-2 border-r-[1.5px] border-b-[1.5px] border-[#6b7280]"
      />
      <div className="mb-1.5 flex items-center justify-between gap-2 border-b border-[#181e27] pb-1">
        <span className="text-[15px] font-medium text-[#e5e7eb]">{title}</span>
        {headerRight}
      </div>
      {children}
    </div>
  );
}

export function SystemDashboard() {
  const {
    identity,
    telemetry,
    fanStatus,
    fanPending,
    fanFeedback,
    backendError,
    selectFanMode,
  } = useSystemBackend();

  const {
    controls,
    controlsError,
    brightnessPercent,
    blueLightLevel,
    volumePercent,
    setBrightnessPercent,
    setBlueLightLevel,
    setVolumePercent,
  } = useDesktopControls();

  const {
    preferences,
    pendingIds: prefPendingIds,
    error: preferencesError,
    togglePreference,
  } = usePreferences();

  const isLiveConnected = Boolean(telemetry?.ready) && !backendError;

  const cpuFanRpm = telemetry?.ready ? telemetry.fans.cpuFanRpm : null;
  const gpuFanRpm = telemetry?.ready ? telemetry.fans.gpuFanRpm : null;

  const ramPercent =
    telemetry?.ready && telemetry.memory.ramTotalMb > 0
      ? telemetry.memory.ramPercent
      : null;
  const ramSubtitle =
    telemetry?.ready && telemetry.memory.ramTotalMb > 0
      ? `${Math.round(telemetry.memory.ramUsedMb / 1024)}/${Math.round(telemetry.memory.ramTotalMb / 1024)}GB`
      : "Unavailable";
  const ramDetail =
    telemetry?.ready && telemetry.memory.ramTotalMb > 0
      ? `Memory: ${(telemetry.memory.ramUsedMb / 1024).toFixed(1)} / ${(telemetry.memory.ramTotalMb / 1024).toFixed(1)} GiB (${Math.round(telemetry.memory.ramPercent)}%)`
      : "Memory telemetry unavailable";

  const diskPercent =
    telemetry?.ready && telemetry.memory.diskTotalGb > 0
      ? telemetry.memory.diskPercent
      : null;
  const diskSubtitle =
    telemetry?.ready && telemetry.memory.diskTotalGb > 0
      ? `${telemetry.memory.diskUsedGb}/${telemetry.memory.diskTotalGb}GB`
      : "Unavailable";
  const diskDetail =
    telemetry?.ready && telemetry.memory.diskTotalGb > 0
      ? `Root storage: ${telemetry.memory.diskUsedGb} / ${telemetry.memory.diskTotalGb} GiB (${Math.round(telemetry.memory.diskPercent)}%)`
      : "Storage telemetry unavailable";

  const gpu = telemetry?.ready ? telemetry.gpu : null;

  let gpuHeaderBadge: ReactNode = null;
  let gpuClock = { text: "--", pct: 0, disabled: true };
  let gpuTemp = { text: "--", pct: 0, disabled: true };
  let gpuUtil = { text: "--", pct: 0, disabled: true };

  if (gpu) {
    switch (gpu.state) {
      case "active": {
        const maxClk = gpu.maxClockMhz > 0 ? gpu.maxClockMhz : 2100;
        gpuHeaderBadge =
          gpu.vramTotalMb > 0 ? (
            <span
              className="text-[10px] text-[#6b7280] tabular-nums"
              title={`VRAM: ${gpu.vramUsedMb} / ${gpu.vramTotalMb} MB`}
            >
              {gpu.vramUsedMb}/{gpu.vramTotalMb} MB
            </span>
          ) : null;
        gpuClock = {
          text: `${gpu.coreClockMhz} MHz`,
          pct: (gpu.coreClockMhz / maxClk) * 100,
          disabled: false,
        };
        gpuTemp = {
          text: `${gpu.tempC} °C`,
          pct: gpu.tempC,
          disabled: false,
        };
        gpuUtil = {
          text: `${gpu.utilizationPct} %`,
          pct: gpu.utilizationPct,
          disabled: false,
        };
        break;
      }
      case "partial":
        gpuHeaderBadge = (
          <span
            className="text-[10px] text-[#6b7280]"
            title={gpu.reason}
          >
            Temp Only
          </span>
        );
        gpuClock = { text: "Unavailable", pct: 0, disabled: true };
        gpuTemp = { text: `${gpu.tempC} °C`, pct: gpu.tempC, disabled: false };
        gpuUtil = { text: "Unavailable", pct: 0, disabled: true };
        break;
      case "sleeping":
        gpuHeaderBadge = (
          <span
            className="text-[10px] text-[#6b7280]"
            title="dGPU runtime suspended to conserve power; polling paused"
          >
            Sleeping
          </span>
        );
        gpuClock = { text: "Sleeping", pct: 0, disabled: true };
        gpuTemp = { text: "Sleeping", pct: 0, disabled: true };
        gpuUtil = { text: "Sleeping", pct: 0, disabled: true };
        break;
      case "unavailable":
        gpuHeaderBadge = (
          <span className="text-[10px] text-[#6b7280]" title={gpu.reason}>
            Unavailable
          </span>
        );
        gpuClock = { text: "Unavailable", pct: 0, disabled: true };
        gpuTemp = { text: "Unavailable", pct: 0, disabled: true };
        gpuUtil = { text: "Unavailable", pct: 0, disabled: true };
        break;
    }
  }

  return (
    <div className="flex min-h-full flex-col justify-between gap-2.5 bg-[#07090c] px-3.5 py-2.5 select-none lg:px-4">
      {/* 3-Column Reference Workspace */}
      <div className="grid flex-1 grid-cols-1 gap-2.5 lg:grid-cols-[1.48fr_1.12fr_0.90fr]">
        {/* LEFT COLUMN: Open Hero Column (System Header + Profiles + CPU Dial + Fan Modes) */}
        <section className="flex flex-col justify-between bg-[#07090c]">
          {/* Header Row: "System" + Live Hardware Specs */}
          <div>
            <div className="flex flex-wrap items-start gap-x-5 gap-y-1">
              <div className="flex items-center gap-2">
                <h2 className="text-[18px] font-normal tracking-wide text-slate-100">
                  System
                </h2>

                {!isLiveConnected && !backendError && (
                  <span className="rounded border border-[#262b36] bg-[#11141a] px-1.5 py-[1px] font-mono text-[9px] tracking-wider text-[#6b7280] uppercase">
                    Stage 2 Preview
                  </span>
                )}

                {(backendError || (isLiveConnected && !fanStatus.daemonConnected)) && (
                  <span
                    title={
                      backendError ??
                      fanStatus.error ??
                      "acer-control-daemon.service unreachable"
                    }
                    className="rounded border border-amber-500/35 bg-amber-950/30 px-1.5 py-[1px] font-mono text-[9px] tracking-wider text-amber-400/90 uppercase"
                  >
                    {backendError ? "Backend Offline" : "Daemon Offline"}
                  </span>
                )}
              </div>

              <div className="space-y-0.5 pt-0.5 text-[11px] leading-tight text-slate-300">
                <div>{identity?.cpuModel ?? "Detecting CPU..."}</div>
                <div className="text-slate-400">
                  {identity?.gpuModel ?? "Detecting GPU..."}
                </div>
              </div>
            </div>

            {/* Subtle top hairline with centered cyan down-chevron matching reference */}
            <div className="relative mt-2 flex items-center">
              <div className="h-[1px] flex-1 bg-gradient-to-r from-[#262b36] via-[#00d8ff]/45 to-[#262b36]" />
              <svg
                viewBox="0 0 16 8"
                className="mx-1 h-2 w-4 shrink-0 text-[#00d8ff]/75"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
                aria-hidden="true"
              >
                <path
                  d="M2 1.5L8 6.5L14 1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
              <div className="h-[1px] flex-1 bg-gradient-to-r from-[#262b36] via-[#00d8ff]/45 to-[#262b36]" />
            </div>

            {/* Performance Profile Pills (Unsupported — Gray & Disabled, Preserved in Layout) */}
            <div className="mt-2.5 flex justify-center">
              <div
                className="flex items-center gap-4 text-[11.5px]"
                title="Power profiles are not supported on this backend"
              >
                <button
                  type="button"
                  disabled
                  tabIndex={-1}
                  aria-disabled="true"
                  className="pointer-events-none cursor-not-allowed px-2 py-0.5 text-[#757e8c] opacity-55"
                >
                  Performance
                </button>
                <button
                  type="button"
                  disabled
                  tabIndex={-1}
                  aria-disabled="true"
                  className="pointer-events-none -skew-x-12 cursor-not-allowed border border-[#1f2630] bg-[#0b0e13] px-3.5 py-0.5 text-[#757e8c] opacity-55"
                >
                  <span className="inline-block skew-x-12">Balance</span>
                </button>
                <button
                  type="button"
                  disabled
                  tabIndex={-1}
                  aria-disabled="true"
                  className="pointer-events-none cursor-not-allowed px-2 py-0.5 text-[#757e8c] opacity-55"
                >
                  Quiet
                </button>
              </div>
            </div>
          </div>

          {/* Centerpiece CPU Hero Speedometer Dial with subtle left-column ambient glow */}
          <div
            className="flex min-h-0 w-full flex-1 items-center justify-center py-0.5"
            style={{
              background:
                "radial-gradient(circle at center, rgba(0, 175, 240, 0.09) 0%, rgba(12, 15, 20, 0) 68%)",
            }}
          >
            <CpuTachometer
              clockMhz={telemetry?.ready ? telemetry.cpu.avgFreqMhz : null}
              maxGhz={identity?.cpuMaxGhz ?? 5}
              utilizationPct={
                telemetry?.ready ? telemetry.cpu.utilizationPct : null
              }
              tempC={telemetry?.ready ? telemetry.cpu.tempC : null}
            />
          </div>

          {/* Bottom Fan Mode Selector Bar */}
          <div className="flex flex-col items-center pt-1 pb-0.5">
            <div className="flex flex-wrap items-center justify-center gap-3 sm:gap-5">
              {FAN_MODES.map((mode) => {
                if (!mode.supported) {
                  return (
                    <button
                      key={mode.id}
                      type="button"
                      disabled
                      tabIndex={-1}
                      aria-disabled="true"
                      title={`${mode.label} Fan Mode — Not supported by backend`}
                      className="pointer-events-none cursor-not-allowed px-3.5 py-0.5 text-[12px] text-[#757e8c] opacity-55"
                    >
                      {mode.label}
                    </button>
                  );
                }

                const isSelected =
                  fanStatus.daemonConnected && fanStatus.mode === mode.id;
                const isInteractive = fanStatus.daemonConnected && !fanPending;

                return (
                  <button
                    key={mode.id}
                    type="button"
                    disabled={!isInteractive}
                    aria-pressed={isSelected}
                    title={
                      fanStatus.daemonConnected
                        ? mode.id === "max"
                          ? "Set cooling fans to Maximum speed (D-Bus)"
                          : "Set cooling fans to Dynamic Auto mode (D-Bus)"
                        : (fanStatus.error ?? "Fan daemon unreachable")
                    }
                    onClick={() => void selectFanMode(mode.id)}
                    className={`-skew-x-12 px-3.5 py-0.5 text-[12px] font-medium transition-all duration-150 active:scale-[0.98] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
                      !fanStatus.daemonConnected
                        ? "cursor-not-allowed text-[#6b7280] opacity-45"
                        : isSelected
                          ? "bg-[#00b8ff] text-[#0a0c10] shadow-[0_0_14px_rgba(0,184,255,0.4)]"
                          : "text-[#7dd3fc] hover:bg-[#00d8ff]/10 hover:text-[#00d8ff]"
                    } ${fanPending ? "opacity-75" : ""}`}
                  >
                    <span className="inline-block skew-x-12">
                      {mode.label}
                    </span>
                  </button>
                );
              })}
            </div>

            <div className="mt-1 min-h-[14px] text-center text-[10px]">
              {fanFeedback ? (
                <span
                  className={
                    fanFeedback.ok ? "text-[#9be000]/90" : "text-amber-400"
                  }
                >
                  {fanFeedback.message}
                </span>
              ) : !fanStatus.daemonConnected && isLiveConnected ? (
                <span className="text-[#6b7280]">
                  Daemon offline — Auto/Max fan controls unavailable
                </span>
              ) : null}
            </div>
          </div>
        </section>

        {/* MIDDLE COLUMN: Memory & Storage, Fan Speed, Preferences */}
        <section className="flex flex-col gap-2.5">
          <DashboardCard title="Memory & Storage">
            <div className="grid grid-cols-2 gap-1.5 py-0.5">
              <MiniRadialGauge
                percent={ramPercent}
                title="Memory"
                subtitle={ramSubtitle}
                detailTitle={ramDetail}
              />
              <MiniRadialGauge
                percent={diskPercent}
                title="Storage"
                subtitle={diskSubtitle}
                detailTitle={diskDetail}
              />
            </div>
          </DashboardCard>

          <DashboardCard
            title="Fan Speed"
            headerRight={
              telemetry?.fans.error && cpuFanRpm === null && gpuFanRpm === null ? (
                <span
                  className="text-[10px] text-amber-400/80"
                  title={telemetry.fans.error}
                >
                  Unavailable
                </span>
              ) : undefined
            }
          >
            <div className="grid grid-cols-2 gap-1.5 py-0.5">
              <MiniRadialGauge
                percent={
                  cpuFanRpm !== null
                    ? Math.min(100, Math.round((cpuFanRpm / 5000) * 100))
                    : null
                }
                title="CPU Fan"
                subtitle={cpuFanRpm !== null ? `${cpuFanRpm} RPM` : "Unavailable"}
                numericValue={
                  cpuFanRpm !== null
                    ? { value: cpuFanRpm, unit: "RPM" }
                    : undefined
                }
                detailTitle={
                  cpuFanRpm !== null
                    ? `CPU Fan: ${cpuFanRpm} RPM`
                    : (telemetry?.fans.error ?? "CPU fan sensor unavailable")
                }
              />
              <MiniRadialGauge
                percent={
                  gpuFanRpm !== null
                    ? Math.min(100, Math.round((gpuFanRpm / 5000) * 100))
                    : null
                }
                title="GPU Fan"
                subtitle={gpuFanRpm !== null ? `${gpuFanRpm} RPM` : "Unavailable"}
                numericValue={
                  gpuFanRpm !== null
                    ? { value: gpuFanRpm, unit: "RPM" }
                    : undefined
                }
                detailTitle={
                  gpuFanRpm !== null
                    ? `GPU Fan: ${gpuFanRpm} RPM`
                    : (telemetry?.fans.error ?? "GPU fan sensor unavailable")
                }
              />
            </div>
          </DashboardCard>

          <DashboardCard
            title="Preferences"
            className="flex flex-1 flex-col"
            headerRight={
              preferencesError ? (
                <span
                  title={preferencesError}
                  className="text-[10px] text-amber-400/90"
                >
                  Sync issue
                </span>
              ) : undefined
            }
          >
            <div className="grid flex-1 grid-cols-3 gap-x-2 gap-y-2 place-items-center py-0.5">
              {PREFERENCE_TILES.map((tile) => {
                const state = preferences?.items.find((i) => i.id === tile.id);
                const label = state?.label ?? tile.fallbackLabel;
                const supported = state?.supported ?? false;
                const isActive = state?.active === true;
                const isPending = Boolean(prefPendingIds[tile.id]);

                if (!supported) {
                  const unavailableTooltip =
                    state?.reason ??
                    `${label} — Checking system support or unsupported on this hardware`;
                  return (
                    <div
                      key={tile.id}
                      title={unavailableTooltip}
                      className="flex w-full cursor-not-allowed justify-center"
                    >
                      <button
                        type="button"
                        disabled
                        tabIndex={-1}
                        aria-disabled="true"
                        aria-label={`${label} (Unsupported)`}
                        className="pointer-events-none flex w-full flex-col items-center justify-center gap-1.5 rounded py-1 text-[#757e8c] opacity-45"
                      >
                        <span className="flex h-10 w-10 items-center justify-center">
                          {renderPreferenceIcon(tile.id, false)}
                        </span>
                        <span className="text-[12.5px] leading-tight tracking-tight text-[#7c8594]">
                          {label}
                        </span>
                      </button>
                    </div>
                  );
                }

                const tooltip = `${label}: ${isActive ? "Active / Enabled" : "Inactive / Disabled"}${
                  state?.reason ? ` — ${state.reason}` : ""
                }`;

                return (
                  <button
                    key={tile.id}
                    type="button"
                    disabled={isPending}
                    aria-pressed={isActive}
                    title={tooltip}
                    onClick={() => void togglePreference(tile.id)}
                    className={`flex w-full cursor-pointer flex-col items-center justify-center gap-1.5 rounded py-1 transition-all duration-150 active:scale-[0.97] focus:outline-none focus-visible:ring-1 focus-visible:ring-[#00d8ff] ${
                      isPending ? "opacity-70" : "hover:bg-[#141922]"
                    } ${
                      isActive
                        ? "text-[#9be000]"
                        : "text-[#aeb6c4] hover:text-white"
                    }`}
                  >
                    <span className="flex h-10 w-10 items-center justify-center">
                      {renderPreferenceIcon(tile.id, isActive)}
                    </span>
                    <span
                      className={`text-[12.5px] leading-tight tracking-tight ${
                        isActive
                          ? "font-medium text-[#9be000]"
                          : "text-[#c4cbd6]"
                      }`}
                    >
                      {label}
                    </span>
                  </button>
                );
              })}
            </div>
          </DashboardCard>
        </section>

        {/* RIGHT COLUMN: GPU Info (top), Configuration (bottom) */}
        <section className="flex flex-col gap-2.5">
          <DashboardCard
            title="GPU Info"
            headerRight={gpuHeaderBadge}
            className="flex flex-[0.82] flex-col justify-between"
          >
            <div className="flex flex-1 flex-col justify-evenly gap-3 py-1.5">
              <SegmentedBar
                label="Clock"
                valueText={gpuClock.text}
                percent={gpuClock.pct}
                disabled={gpuClock.disabled}
              />
              <SegmentedBar
                label="Temperature"
                valueText={gpuTemp.text}
                percent={gpuTemp.pct}
                disabled={gpuTemp.disabled}
              />
              <SegmentedBar
                label="Utilization"
                valueText={gpuUtil.text}
                percent={gpuUtil.pct}
                disabled={gpuUtil.disabled}
              />
            </div>
          </DashboardCard>

          <DashboardCard
            title="Configuration"
            headerRight={
              controlsError ? (
                <span
                  className="max-w-[115px] truncate text-[10px] text-amber-400/90"
                  title={controlsError}
                >
                  {controlsError}
                </span>
              ) : undefined
            }
            className="flex flex-[1.18] flex-col justify-between"
          >
            <div className="flex flex-1 flex-col justify-evenly gap-2.5 py-1.5">
              {(() => {
                const blueView = describeBlueLightControl(
                  controls?.blueLight,
                  blueLightLevel
                );
                const brightness = controls?.brightness;
                const brightAvailable = Boolean(brightness?.available);
                const brightText = brightAvailable
                  ? `${brightnessPercent}%`
                  : "Unavailable";
                const brightTooltip = brightAvailable
                  ? `Display brightness (${brightness?.connector ?? "internal"} via ${brightness?.backend}): ${brightnessPercent}% (${brightness?.value}/${brightness?.max})`
                  : (brightness?.error ?? "Display backlight unavailable");

                const volume = controls?.volume;
                const volAvailable = Boolean(volume?.available);
                const volText = !volAvailable
                  ? "Unavailable"
                  : volume?.muted
                    ? `Muted (${volumePercent}%)`
                    : `${volumePercent}%`;
                const volTooltip = !volAvailable
                  ? (volume?.error ?? "Audio output device unavailable")
                  : `Output: ${volume?.sinkDescription ?? volume?.sinkName ?? "Default Sink"} (${volText})`;

                return (
                  <>
                    <ConfigSliderRow
                      icon={<BlueLightIcon />}
                      label="Blue Light Filter"
                      valueText={blueView.valueText}
                      minLabel="0"
                      maxLabel="10"
                      percent={blueView.percent}
                      disabled={!blueView.available}
                      trackVariant={blueView.trackVariant}
                      stepPercent={5}
                      ariaMin={0}
                      ariaMax={10}
                      ariaValueNow={blueLightLevel}
                      ariaValueText={blueView.valueText}
                      tooltip={blueView.tooltip}
                      onChange={(pct, commit) =>
                        setBlueLightLevel((pct / 100) * 10, commit)
                      }
                    />

                    <ConfigSliderRow
                      icon={<BrightnessIcon />}
                      label="Brightness"
                      valueText={brightText}
                      minLabel="0"
                      maxLabel="100"
                      percent={brightAvailable ? brightnessPercent : 0}
                      disabled={!brightAvailable}
                      stepPercent={2}
                      ariaMin={0}
                      ariaMax={100}
                      ariaValueNow={brightnessPercent}
                      ariaValueText={brightText}
                      tooltip={brightTooltip}
                      onChange={(pct, commit) =>
                        setBrightnessPercent(pct, commit)
                      }
                    />

                    <ConfigSliderRow
                      icon={<VolumeIcon />}
                      label="Volume"
                      valueText={volText}
                      minLabel="0"
                      maxLabel="100"
                      percent={volAvailable ? volumePercent : 0}
                      disabled={!volAvailable}
                      mutedTrack={Boolean(volAvailable && volume?.muted)}
                      stepPercent={2}
                      ariaMin={0}
                      ariaMax={100}
                      ariaValueNow={volumePercent}
                      ariaValueText={volText}
                      tooltip={volTooltip}
                      onChange={(pct, commit) => setVolumePercent(pct, commit)}
                    />
                  </>
                );
              })()}
            </div>
          </DashboardCard>
        </section>
      </div>
    </div>
  );
}
