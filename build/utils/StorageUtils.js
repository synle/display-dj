"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const { app } = require('electron');
const fs = require('fs');
const path = require('path');
console.log('>> app', app);
const appDataPath = app.getPath('appData');
console.log('>> appDataPath', appDataPath);
function _getPath(fileName) {
    const baseDir = path.join(appDataPath, 'display-dj');
    return path.join(baseDir, fileName);
}
const StorageUtils = {
    readJSON: (file) => {
        file = _getPath(file);
        try {
            return JSON.parse(fs.readFileSync(file, { encoding: 'utf8', flag: 'r' }).trim());
        }
        catch (err) {
            return {};
        }
    },
    writeJSON: (file, jsonObject) => {
        file = _getPath(file);
        fs.writeFileSync(file, JSON.stringify(jsonObject, null, 2));
    },
};
exports.default = StorageUtils;
//# sourceMappingURL=StorageUtils.js.map