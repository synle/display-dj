export interface Monitor {
  id: string;
  uid: string;
  name: string;
  originalName: string;
  brightness: number;
  contrast: number | null;
  supportsBrightness: boolean;
  isBuiltIn: boolean;
  hidden: boolean;
}

export interface MonitorMetadata {
  uid: string;
  apiId: string;
  apiName: string;
  label: string;
  sortOrder: number;
  hidden: boolean;
}

export interface NightModeSchedule {
  enabled: boolean;
  nightStart: string;
  nightBrightness: number;
  dayStart: string;
  dayBrightness: number;
}

export interface TilingPreferences {
  enabled: boolean;
  halfRatio: number;
  thirdRatio: number;
  gap: number;
  sideEdgeTrigger: number;
  topEdgeTrigger: number;
  cornerTrigger: number;
  exposeEnabled: boolean;
  exposeColumns: number;
  exposeRows: number;
}

/** A single rule within a layout preset: match windows by app name and apply a tiling layout. */
export interface LayoutRule {
  appMatch: string;
  layout: string;
  displayIndex: number | null;
}

/** A named window layout preset containing one or more layout rules. */
export interface LayoutPreset {
  name: string;
  rules: LayoutRule[];
}

export interface Preferences {
  showIndividualDisplays: boolean;
  minBrightness: number;
  keyBindings: KeyBinding[];
  profiles: Profile[];
  nightModeSchedule: NightModeSchedule;
  showContrast: boolean;
  debugLogging: boolean;
  launchAtLogin: boolean;
  monitorConfigs: MonitorMetadata[];
  tiling: TilingPreferences;
  layoutPresets: LayoutPreset[];
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}

export interface Profile {
  name: string;
  command: string | string[];
}
