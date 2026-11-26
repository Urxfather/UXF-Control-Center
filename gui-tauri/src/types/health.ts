export type HealthStatus = "passed" | "warning" | "failed" | "info";

export interface HealthItem {
  id: string;
  name: string;
  status: HealthStatus;
  summary: string;
  detail: string;
  repair_action?: string | null;
}

export interface SystemHealthReport {
  overall_status: HealthStatus;
  hardware_model: string;
  hardware_compatible: boolean;
  kernel_version: string;
  items: HealthItem[];
}

export interface UpdateCheckResult {
  current_version: string;
  latest_version?: string | null;
  update_available: boolean;
  download_url?: string | null;
  release_notes?: string | null;
  update_command: string;
  has_dnf_repo: boolean;
  error?: string | null;
}
