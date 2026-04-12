export interface Monitor {
  id: string;
  name: string;
  brightness: number;
  contrast: number;
  supportsBrightness: boolean;
  supportsContrast: boolean;
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
  keyBindings: KeyBinding[];
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}
