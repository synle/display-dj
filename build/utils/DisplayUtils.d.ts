import { MonitorUpdateInput } from '../index.d';
declare const DisplayUtils: {
    getMonitors: () => Promise<Required<MonitorUpdateInput>[]>;
    updateMonitor: (monitor: MonitorUpdateInput) => Promise<void>;
    toggleDarkMode: (isDarkModeOn: boolean) => Promise<string>;
};
export default DisplayUtils;
