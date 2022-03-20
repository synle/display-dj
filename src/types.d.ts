type DisplayType = 'laptop_monitor' | 'external_monitor' | 'unknown_monitor';

type KeyBinding = {
  key: string;
  command: // brightness commands
  | 'command/changeBrightness/down'
    | 'command/changeBrightness/up'
    | 'command/changeBrightness/0'
    | 'command/changeBrightness/50'
    | 'command/changeBrightness/100'
    // dark mode commands
    | 'command/changeDarkMode/toggle'
    | 'command/changeDarkMode/dark'
    | 'command/changeDarkMode/light';
};

export type SingleMonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
  disabled?: boolean;
  sortOrder?: number;
  type: DisplayType;
};

export type BatchMonitorUpdateInput = {
  brightness?: number;
};

export type Monitor = Required<SingleMonitorUpdateInput>;

export type AppConfig = {
  monitors: Monitor[];
  darkMode: boolean;
  env: string;
  version: string;
};

export type IDisplayAdapter = {
  getMonitorList: () => Promise<string[]>;
  getMonitorType: (targetMonitorId: string) => Promise<DisplayType>;
  getMonitorBrightness: (targetMonitorId: string) => Promise<number>;
  updateMonitorBrightness: (targetMonitorId: string, newBrightness: number) => Promise<void>;
  batchUpdateMonitorBrightness: (monitors: Monitor[]) => Promise<void>;
  getDarkMode: () => Promise<boolean>;
  updateDarkMode: (isDarkModeOn: boolean) => Promise<void>;
};

export type Preference = {
  brightnessBinary: string;
  ddcctlBinary: string;
  showIndividualDisplays: boolean;
  brightnessDelta: number;
  keyBindings: KeyBinding[];
};

export type UIAppState = {
  isUpdatingBrightness: boolean;
};
