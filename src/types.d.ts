type DisplayType = 'laptop_monitor' | 'external_monitor' | 'unknown_monitor';

export type MonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
  disabled?: boolean;
  sortOrder?: number;
  type: DisplayType;
};

export type Monitor = Required<MonitorUpdateInput>;

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
  getDarkMode: () => Promise<boolean>;
  updateDarkMode: (isDarkModeOn: boolean) => Promise<void>;
};

type KeyBinding = {
  key: string;
  command:
    | 'command/changeBrightness/Down'
    | 'command/changeBrightness/Up'
    | 'command/changeBrightness/0'
    | 'command/changeBrightness/50'
    | 'command/changeBrightness/100';
};

export type Preference = {
  ddcctlBinary: string;
  showIndividualDisplays: boolean;
  brightnessDelta: number;
  keyBindings: KeyBinding[];
};
