export interface Monitor {
  id: string;
  name: string;
  originalName: string;
  brightness: number;
  supportsBrightness: boolean;
  isBuiltIn: boolean;
}

export interface MonitorConfig {
  id: string;
  name: string;
  sortOrder: number;
  disabled: boolean;
}

export interface NightModeSchedule {
  enabled: boolean;
  nightStart: string;
  nightBrightness: number;
  dayStart: string;
  dayBrightness: number;
}

export interface Preferences {
  showIndividualDisplays: boolean;
  minBrightness: number;
  keyBindings: KeyBinding[];
  profiles: Profile[];
  nightModeSchedule: NightModeSchedule;
  debugLogging: boolean;
  launchAtLogin: boolean;
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}

export interface Profile {
  name: string;
  command: string | string[];
}
