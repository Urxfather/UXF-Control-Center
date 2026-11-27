export type NavSectionId =
  | "system"
  | "keyboard"
  | "flexikey"
  | "flexicharger"
  | "system-center";

export type SystemCenterTabId =
  | "overview"
  | "health"
  | "updates"
  | "system-info"
  | "about";

export interface NavSection {
  id: Exclude<NavSectionId, "system-center">;
  label: string;
  placeholderHeading: string;
  placeholderNote: string;
}

export const DEFAULT_SECTION: NavSectionId = "system";

export const NAV_SECTIONS: readonly NavSection[] = [
  {
    id: "system",
    label: "System",
    placeholderHeading: "System Dashboard — Coming in Phase 3",
    placeholderNote:
      "Telemetry, cooling, and system status panels will be implemented in the next migration phase.",
  },
  {
    id: "keyboard",
    label: "LED Keyboard",
    placeholderHeading: "LED Keyboard — Coming Soon",
    placeholderNote:
      "RGB backlight zones, lighting modes, and brightness controls are scheduled for a later phase.",
  },
  {
    id: "flexikey",
    label: "Flexikey",
    placeholderHeading: "Flexikey — Coming Soon",
    placeholderNote:
      "Key mapping and macro configuration support is not yet available in this release.",
  },
  {
    id: "flexicharger",
    label: "FlexiCharger",
    placeholderHeading: "FlexiCharger — Coming Soon",
    placeholderNote:
      "Battery charge threshold controls are not yet supported on this platform.",
  },
];

export const SYSTEM_CENTER_TABS: readonly {
  id: SystemCenterTabId;
  label: string;
}[] = [
  { id: "overview", label: "Overview" },
  { id: "health", label: "Health" },
  { id: "updates", label: "Updates" },
  { id: "system-info", label: "System Info" },
  { id: "about", label: "About" },
];
