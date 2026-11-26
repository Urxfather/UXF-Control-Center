export interface PreferenceControlState {
  id: string;
  label: string;
  supported: boolean;
  active: boolean | null;
  source: string;
  reason: string | null;
}

export interface PreferencesState {
  items: PreferenceControlState[];
  sampledAtMs: number;
}
