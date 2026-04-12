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
  brightnessDelta: number;
  contrastDelta: number;
  minBrightness: number;
  keyBindings: KeyBinding[];
  nightModeSchedule: NightModeSchedule;
  debugLogging: boolean;
  launchAtLogin: boolean;
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}
