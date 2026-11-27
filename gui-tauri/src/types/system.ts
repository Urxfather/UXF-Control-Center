export interface SystemIdentity {
  cpuModel: string;
  gpuModel: string;
  cpuMaxGhz: number;
}

export interface CpuTelemetry {
  avgFreqMhz: number;
  maxFreqMhz: number;
  utilizationPct: number;
  tempC: number | null;
  tempSource: string;
}

export type GpuTelemetry =
  | {
      state: "active";
      coreClockMhz: number;
      maxClockMhz: number;
      tempC: number;
      utilizationPct: number;
      vramUsedMb: number;
      vramTotalMb: number;
    }
  | {
      state: "partial";
      tempC: number;
      reason: string;
    }
  | {
      state: "sleeping";
    }
  | {
      state: "unavailable";
      reason: string;
    };

export interface SystemMemory {
  ramUsedMb: number;
  ramTotalMb: number;
  ramPercent: number;
  diskUsedGb: number;
  diskTotalGb: number;
  diskPercent: number;
}

export interface FanTelemetry {
  cpuFanRpm: number | null;
  gpuFanRpm: number | null;
  error: string | null;
}

export interface TelemetrySnapshot {
  cpu: CpuTelemetry;
  gpu: GpuTelemetry;
  memory: SystemMemory;
  fans: FanTelemetry;
  sampledAtMs: number;
  ready: boolean;
}

export interface FanStatus {
  mode: "auto" | "max" | "unavailable";
  daemonConnected: boolean;
  daemonVersion: string | null;
  error: string | null;
}

export interface BrightnessState {
  available: boolean;
  percent: number;
  value: number;
  min: number;
  max: number;
  connector: string | null;
  backend: "mutter" | "login1" | "unavailable";
  error: string | null;
}

export type BlueLightStatusReason =
  | "active"
  | "disabled"
  | "schedule_inactive"
  | "paused_until_tomorrow"
  | "unavailable";

export interface BlueLightState {
  available: boolean;
  level: number;
  kelvin: number;
  minKelvin: number;
  maxKelvin: number;
  enabled: boolean;
  active: boolean;
  scheduleAutomatic: boolean;
  scheduleFrom: number;
  scheduleTo: number;
  statusReason: BlueLightStatusReason;
  error: string | null;
}

export interface VolumeState {
  available: boolean;
  percent: number;
  muted: boolean;
  sinkName: string | null;
  sinkDescription: string | null;
  error: string | null;
}

export interface DesktopControlsState {
  brightness: BrightnessState;
  blueLight: BlueLightState;
  volume: VolumeState;
}

export type KeyboardEffectMode = "static" | "breathing" | "rainbow";

export interface KeyboardState {
  available: boolean;
  daemonConnected: boolean;
  daemonVersion: string | null;
  daemonSupportsFullSync: boolean;
  brightnessStep: number;
  brightnessRaw: number;
  brightnessTarget: number;
  maxBrightness: number;
  color: [number, number, number];
  effect: KeyboardEffectMode;
  sleepTimerEnabled: boolean;
  sleepTimerSeconds: number;
  idleDimmed: boolean;
  idleStatus: string;
  idleMonitorSupported: boolean;
  fadeFrom: number;
  fadeTo: number;
  fadeStartMonoMs: number;
  fadeDurationMs: number;
  animEpochMs: number;
  monoNowMs: number;
  breathingSupported: boolean;
  rainbowSupported: boolean;
  rainbowReason: string | null;
  bootEffectSupported: boolean;
  bootEffectReason: string | null;
  zonesLabel: string;
  error: string | null;
}
