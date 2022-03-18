import { DISPLAY_TYPE } from 'src/constants';

export type MonitorUpdateInput = {
  id: string;
  name?: string;
  brightness?: number;
  disabled?: boolean;
  sortOrder?: number;
  type: DISPLAY_TYPE;
};

export type Monitor = Required<MonitorUpdateInput>;

export type AppConfig = {
  monitors: Monitor[];
  darkMode: boolean;
  env: string;
  version: string;
};

export type DisplayAdapter = {
  getMonitorList: () => Promise<string[]>;
  getMonitorType: (idToUse: string) => Promise<string>;
  getMonitorBrightness: (idToUse: string) => Promise<number>;
  updateMonitorBrightness: (monitorId: string, newBrightness: number) => Promise<void>;
  getDarkMode: () => Promise<boolean>;
  updateDarkMode: (isDarkModeOn: boolean) => Promise<void>;
};
