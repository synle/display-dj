type DisplayType = 'laptop_monitor' | 'external_monitor' | 'unknown_monitor';

type Command =
  // reset
  | 'command/reset'
  // brightness commands
  | 'command/changeBrightness/down'
  | 'command/changeBrightness/up'
  | 'command/changeBrightness/0'
  | 'command/changeBrightness/5'
  | 'command/changeBrightness/10'
  | 'command/changeBrightness/50'
  | 'command/changeBrightness/100'
  // volumes commands
  | 'command/changeVolume/0'
  | 'command/changeVolume/50'
  | 'command/changeVolume/100'
  // dark mode commands
  | 'command/changeDarkMode/toggle'
  | 'command/changeDarkMode/dark'
  | 'command/changeDarkMode/light'
  // open external links or text files
  | 'command/openExternal/file/monitorConfigs'
  | 'command/openExternal/file/preferences'
  | 'command/openExternal/file/devLogs'
  | 'command/openExternal/link/bugReport'
  | 'command/openExternal/link/aboutUs'
  | string;

type BrightnessPreset = {
  // TODO: this is not supported, preset will apply to all monitors...
  // which?: string;
  /**
   * whether or not this brightness preset will be associated with the dark or light mode
   */
  syncedWithMode?: 'light' | 'dark';
  level: number;
};

type VolumePreset = {
  level: number;
};

type KeyBinding = {
  key: string;
  command: Command[] | Command;
  notification?: string;
};

export type SingleMonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
  disabled?: boolean;
  sortOrder?: number;
  type?: DisplayType;
};

export type BatchMonitorUpdateInput = {
  brightness?: number;
};

export type Monitor = Required<SingleMonitorUpdateInput>;

export type VolumeInput = {
  muted?: boolean;
  value?: number;
};

export type Volume = Required<VolumeInput> & { isDisabled?: boolean };

export type TimeOfDayDarkMode = {
  fromHour: number;
  toHour: number;
}

export type AppConfig = {
  monitors: Monitor[];
  darkMode: boolean;
  volume: Volume;
  env: string;
  version: string;
  platform: 'win32' | 'darwin';
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
  showIndividualDisplays: boolean;
  logging: boolean;
  brightnessDelta: number;
  brightnessPresets: BrightnessPreset[];
  volumePresets: VolumePreset[];
  keyBindings: KeyBinding[];
  timeOfDayDarkMode?: TimeOfDayDarkMode;
};

export type UIAppState = {};
