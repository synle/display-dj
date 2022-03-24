type DisplayType = 'laptop_monitor' | 'external_monitor' | 'unknown_monitor';

type BrightnessCommand = // brightness commands
  | 'command/changeBrightness/down'
  | 'command/changeBrightness/up'
  | 'command/changeBrightness/0'
  | 'command/changeBrightness/10'
  | 'command/changeBrightness/50'
  | 'command/changeBrightness/100'

type Command =
  // reset
  | 'command/reset'
  // brightness commands
  | BrightnessCommand
  // dark mode commands
  | 'command/changeDarkMode/toggle'
  | 'command/changeDarkMode/dark'
  | 'command/changeDarkMode/light'
  // open external links or text files
  | 'command/openExternal/file/monitorConfigs'
  | 'command/openExternal/file/preferences'
  | 'command/openExternal/file/devLogs'
  | 'command/openExternal/link/bugReport'
  | 'command/openExternal/link/aboutUs';

type BrightnessPreset = {
  which?: string;
  level: number;
}

type KeyBinding = {
  key: string;
  command: Command[] | Command;
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
  brightnessPresets: BrightnessPreset[];
  keyBindings: KeyBinding[];
};

export type UIAppState = {};
