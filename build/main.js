"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    Object.defineProperty(o, k2, { enumerable: true, get: function() { return m[k]; } });
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || function (mod) {
    if (mod && mod.__esModule) return mod;
    var result = {};
    if (mod != null) for (var k in mod) if (k !== "default" && Object.prototype.hasOwnProperty.call(mod, k)) __createBinding(result, mod, k);
    __setModuleDefault(result, mod);
    return result;
};
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
Object.defineProperty(exports, "__esModule", { value: true });
const electron_1 = require("electron");
const react_router_dom_1 = require("react-router-dom");
const path = __importStar(require("path"));
const Endpoints_1 = require("./utils/Endpoints");
function createWindow() {
    const mainWindow = new electron_1.BrowserWindow({
        height: 600,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false,
        },
        width: 800,
    });
    mainWindow.loadFile(path.join(__dirname, '../index.html'));
    mainWindow.webContents.openDevTools();
}
electron_1.app.on('ready', () => {
    (0, Endpoints_1.setUpDataEndpoints)();
    createWindow();
    electron_1.app.on('activate', function () {
        if (electron_1.BrowserWindow.getAllWindows().length === 0)
            createWindow();
    });
});
electron_1.app.on('window-all-closed', () => electron_1.app.quit());
const _apiCache = {};
electron_1.ipcMain.on('mainAppEvent/fetch', (event, data) => __awaiter(void 0, void 0, void 0, function* () {
    const { requestId, url, options } = data;
    const responseId = `server response ${Date.now()}`;
    const method = (options.method || 'get').toLowerCase();
    const sessionId = (options === null || options === void 0 ? void 0 : options.headers['api-call-session-id']) || 'display-dj-default-session';
    let body = {};
    try {
        body = JSON.parse(options.body);
    }
    catch (err) { }
    console.log('>> Request', method, url, sessionId, body);
    let matchedUrlObject;
    const matchCurrentUrlAgainst = (matchAgainstUrl) => {
        try {
            return (0, react_router_dom_1.matchPath)(matchAgainstUrl, url);
        }
        catch (err) {
            return undefined;
        }
    };
    try {
        let returnedResponseHeaders = [];
        const sendResponse = (responseData = '', status = 200) => {
            let ok = true;
            if (status >= 300 || status < 200) {
                ok = false;
            }
            console.log('>> Response', status, method, url, sessionId, body, responseData);
            event.reply(requestId, {
                ok,
                status,
                text: JSON.stringify(responseData),
                headers: returnedResponseHeaders,
            });
        };
        const res = {
            status: (code) => {
                return {
                    send: (msg) => {
                        sendResponse(msg, code);
                    },
                    json: (returnedData) => {
                        sendResponse(returnedData, code);
                    },
                };
            },
            header: (key, value) => {
                returnedResponseHeaders.push([key, value]);
            },
        };
        const endpoints = (0, Endpoints_1.getEndpointHandlers)();
        for (const endpoint of endpoints) {
            const [targetMethod, targetUrl, targetHandler] = endpoint;
            const matchedUrlObject = matchCurrentUrlAgainst(targetUrl);
            if (targetMethod === method && matchedUrlObject) {
                const apiCache = {
                    get(key) {
                        try {
                            return _apiCache[sessionId][key];
                        }
                        catch (err) {
                            return undefined;
                        }
                    },
                    set(key, value) {
                        try {
                            _apiCache[sessionId] = _apiCache[sessionId] || {};
                            _apiCache[sessionId][key] = value;
                        }
                        catch (err) { }
                    },
                    json() {
                        return JSON.stringify(_apiCache);
                    },
                };
                const req = {
                    params: matchedUrlObject === null || matchedUrlObject === void 0 ? void 0 : matchedUrlObject.params,
                    body: body,
                    headers: {
                        ['api-call-session-id']: sessionId,
                    },
                };
                return targetHandler(req, res, apiCache);
            }
        }
        sendResponse('Resource Not Found...', 500);
    }
    catch (err) {
        console.log('error', err);
    }
}));
//# sourceMappingURL=main.js.map