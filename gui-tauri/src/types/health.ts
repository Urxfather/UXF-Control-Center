export type HealthStatus = "passed" | "warning" | "failed" | "info";

export interface HealthSubsystemCard {
  id: string;
  title: string;
  status: HealthStatus;
  status_label: string;
  description: string;
}

export interface HealthItem {
  id: string;
  name: string;
  status: HealthStatus;
  summary: string;
  detail: string;
  repair_action?: string | null;
  checked_at_ms: number;
}

export interface SystemHealthReport {
  overall_status: HealthStatus;
  hardware_model: string;
  hardware_compatible: boolean;
  kernel_version: string;
  checked_at_ms: number;
  healthy_count: number;
  total_count: number;
  daemon_version?: string | null;
  daemon_pid?: number | null;
  acpi_interface?: string | null;
  subsystems: HealthSubsystemCard[];
  items: HealthItem[];
}

export interface ReleaseHistoryItem {
  version: string;
  name: string;
  published_at?: string | null;
  is_installed: boolean;
  notes_summary?: string | null;
  html_url?: string | null;
}

export interface UpdateCheckResult {
  current_version: string;
  latest_version?: string | null;
  update_available: boolean;
  download_url?: string | null;
  release_notes?: string | null;
  release_name?: string | null;
  published_at?: string | null;
  asset_size_bytes?: number | null;
  update_command: string;
  has_dnf_repo: boolean;
  checked_at_ms: number;
  history: ReleaseHistoryItem[];
  error?: string | null;
}

export interface ActivityLogEntry {
  id: string;
  message: string;
  source: string;
  tone: "healthy" | "info" | "warning" | "error";
  timestampMs: number;
  timeLabel?: string | null;
}

export interface CachedHealthEnvelope {
  report: SystemHealthReport | null;
  cached: boolean;
  refreshing: boolean;
  checkedAtMs: number | null;
}

export interface CachedUpdatesEnvelope {
  result: UpdateCheckResult | null;
  cached: boolean;
  checking: boolean;
  checkedAtMs: number | null;
}

export interface SystemCenterOverviewSnapshot {
  appVersion: string;
  uptimeSeconds: number | null;
  health: CachedHealthEnvelope;
  updates: CachedUpdatesEnvelope;
  activity: ActivityLogEntry[];
}

export interface HardwareBiosInfo {
  manufacturer: string;
  productModel: string;
  boardName: string;
  biosVendor: string;
  biosVersion: string;
  biosDate: string;
  bootMode: string;
  secureBoot: string;
}

export interface OsKernelInfo {
  distribution: string;
  kernelRelease: string;
  architecture: string;
  displayServer: string;
  hostname: string;
  uptimeSeconds: number | null;
}

export interface ProcessorGraphicsInfo {
  cpuModel: string;
  physicalCores: number | null;
  logicalThreads: number | null;
  maxClockGhz: number | null;
  discreteGpu: string;
  discreteGpuDriver: string | null;
  integratedGpu: string;
  integratedGpuDriver: string | null;
}

export interface MemoryStorageInfo {
  ramTotalMb: number;
  ramTotalDisplay: string;
  swapDisplay: string;
  primaryDrive: string;
  rootFilesystem: string;
}

export interface SystemInformationReport {
  hardware: HardwareBiosInfo;
  osKernel: OsKernelInfo;
  processorGraphics: ProcessorGraphicsInfo;
  memoryStorage: MemoryStorageInfo;
  collectedAtMs: number;
}
