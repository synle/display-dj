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

export type Preference = {
  ddcctlBinary: string;
  showIndividualDisplays: boolean;
};
