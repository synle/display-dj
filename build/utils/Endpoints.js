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
exports.setUpDataEndpoints = exports.getEndpointHandlers = void 0;
const DisplayUtils_1 = __importDefault(require("./DisplayUtils"));
const electronEndpointHandlers = [];
function addDataEndpoint(method, url, incomingHandler) {
    const handlerToUse = (req, res, cache) => __awaiter(this, void 0, void 0, function* () {
        try {
            res.header('api-call-session-id', req.headers['api-call-session-id']);
            yield incomingHandler(req, res, cache);
        }
        catch (err) {
            console.log('err', err);
            res.status(500).send(err);
        }
    });
    electronEndpointHandlers.push([method, url, handlerToUse]);
}
function getEndpointHandlers() {
    return electronEndpointHandlers;
}
exports.getEndpointHandlers = getEndpointHandlers;
function setUpDataEndpoints() {
    addDataEndpoint('get', '/api/monitors', (req, res) => __awaiter(this, void 0, void 0, function* () {
        const monitors = yield DisplayUtils_1.default.getMonitors();
        res.status(200).json(monitors);
    }));
    addDataEndpoint('put', '/api/monitors/:monitorId', (req, res) => __awaiter(this, void 0, void 0, function* () {
        var _a, _b;
        try {
            res.status(200).json(yield DisplayUtils_1.default.updateMonitor({
                id: req.params.monitorId,
                name: (_a = req.body) === null || _a === void 0 ? void 0 : _a.name,
                brightness: (_b = req.body) === null || _b === void 0 ? void 0 : _b.brightness,
            }));
        }
        catch (err) {
            res.status(500).json({ error: `Failed to save monitor`, stack: err });
        }
    }));
    addDataEndpoint('put', '/api/darkMode', (req, res) => __awaiter(this, void 0, void 0, function* () {
        var _c;
        try {
            const isDarkModeOn = ((_c = req.body) === null || _c === void 0 ? void 0 : _c.darkMode) === true;
            res.status(200).json(yield DisplayUtils_1.default.toggleDarkMode(isDarkModeOn));
        }
        catch (err) {
            res.status(500).json({ error: `Failed to update darkMode`, stack: err });
        }
    }));
}
exports.setUpDataEndpoints = setUpDataEndpoints;
//# sourceMappingURL=Endpoints.js.map