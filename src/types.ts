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

export interface Preferences {
  showIndividualDisplays: boolean;
  brightnessDelta: number;
  contrastDelta: number;
  minBrightness: number;
  keyBindings: KeyBinding[];
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}
