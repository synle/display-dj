// TODO: TBD and to be implemented
import { DISPLAY_TYPE } from 'src/constants';
import { MonitorUpdateInput, DisplayAdapter } from 'src/types.d';

const DisplayUtils : DisplayAdapter= {
  getMonitorList: async() => {
    return ['Laptop'];
  },
  getMonitorType: async (idToUse: string) => {
    return DISPLAY_TYPE.LAPTOP;
  },
  getMonitorBrightness: async (idToUse: string) => {
    return 50;
  },
  updateMonitorBrightness: async (monitorId: string, newBrightness: number) => {
  },
  getDarkMode: async () => {
    return true;
  },
  updateDarkMode: async (isDarkModeOn: boolean) => {
  },
};

export default DisplayUtils;
