export interface Monitor {
  id: string;
  name: string;
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
  keyBindings: KeyBinding[];
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}
