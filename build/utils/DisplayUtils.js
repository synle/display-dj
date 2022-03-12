"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const ddcci = require('@hensm/ddcci');
const StorageUtils_1 = __importDefault(require("./StorageUtils"));
const MONITOR_CONFIG_FILE_DIR = 'monitor-configs';
const DisplayUtils = {
    getMonitors: () => __awaiter(void 0, void 0, void 0, function* () {
        const monitors = [];
        const monitorIds = ddcci.getMonitorList();
        const monitorsFromStorage = StorageUtils_1.default.readJSON(MONITOR_CONFIG_FILE_DIR);
        for (const id of monitorIds) {
            monitors.push({
                id,
                name: monitorsFromStorage[id].name || id,
                brightness: yield ddcci.getBrightness(id),
            });
        }
        StorageUtils_1.default.writeJSON(MONITOR_CONFIG_FILE_DIR, monitors);
        return Promise.resolve(monitors);
    }),
    updateMonitor: (monitor) => __awaiter(void 0, void 0, void 0, function* () {
        const monitorsFromStorage = StorageUtils_1.default.readJSON(MONITOR_CONFIG_FILE_DIR);
        if (!monitor.id) {
            throw `id is required.`;
        }
        if (monitor.name) {
        }
        if (monitor.brightness) {
            yield ddcci.setBrightness(monitor.id, monitor.brightness);
        }
    }),
    toggleDarkMode: (isDarkModeOn) => __awaiter(void 0, void 0, void 0, function* () {
        let shellToRun = `Set-ItemProperty -Path HKCU:/SOFTWARE/Microsoft/Windows/CurrentVersion/Themes/Personalize -Name AppsUseLightTheme -Value `.replace(/\//g, '\\');
        shellToRun += isDarkModeOn ? '1' : '0';
        console.log(shellToRun);
        return shellToRun;
    }),
};
exports.default = DisplayUtils;
//# sourceMappingURL=DisplayUtils.js.map