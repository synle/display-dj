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
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}

export interface Profile {
  name: string;
  command: string | string[];
}
