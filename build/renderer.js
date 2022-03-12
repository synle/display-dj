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
var __rest = (this && this.__rest) || function (s, e) {
    var t = {};
    for (var p in s) if (Object.prototype.hasOwnProperty.call(s, p) && e.indexOf(p) < 0)
        t[p] = s[p];
    if (s != null && typeof Object.getOwnPropertySymbols === "function")
        for (var i = 0, p = Object.getOwnPropertySymbols(s); i < p.length; i++) {
            if (e.indexOf(p[i]) < 0 && Object.prototype.propertyIsEnumerable.call(s, p[i]))
                t[p[i]] = s[p[i]];
        }
    return t;
};
const { ipcRenderer } = require('electron');
const origFetch = window.fetch;
window.fetch = (url, options) => {
    if (url.indexOf('/api') !== 0) {
        return origFetch(url, options);
    }
    options = options || {};
    options.headers = options.headers || {};
    return new Promise((resolve, reject) => {
        const requestId = `fetch.requestId.${Date.now()}.${Math.floor(Math.random() * 10000000000000000)}`;
        ipcRenderer.once(requestId, (event, data) => {
            const { ok, text, status, headers } = data;
            let returnedData = text;
            try {
                returnedData = JSON.parse(text);
            }
            catch (err) { }
            console.log('>> Network', ok ? 'Success' : 'Error:', status, options.method || 'get', url, returnedData, headers);
            resolve({
                ok,
                text: () => text,
                headers,
            });
        });
        ipcRenderer.send('mainAppEvent/fetch', { requestId, url, options });
    });
};
function _fetch(input, initOptions) {
    let _a = initOptions || {}, { headers } = _a, restInput = __rest(_a, ["headers"]);
    headers = headers || {};
    headers = Object.assign(Object.assign({}, headers), {
        'Content-Type': 'application/json',
        Accept: 'application/json',
    });
    restInput = restInput || {};
    return fetch(input, Object.assign(Object.assign({}, restInput), { headers }))
        .then((r) => __awaiter(this, void 0, void 0, function* () {
        const response = yield r.text();
        let responseToUse;
        try {
            responseToUse = JSON.parse(response);
        }
        catch (err) {
            responseToUse = response;
        }
        return r.ok ? responseToUse : Promise.reject(responseToUse);
    }))
        .then((r) => {
        const res = r;
        return res;
    });
}
window.ApiUtils = {
    getMonitors: () => _fetch(`/api/monitors`),
    updateMonitor: (monitor) => _fetch(`/api/monitors/${monitor.id}`, {
        method: 'post',
        body: JSON.stringify({
            id: monitor.id,
            name: monitor.name,
            brightness: monitor.brightness,
        }),
    }),
    toggleDarkMode: (darkMode) => _fetch(`/api/darkMode`, {
        method: 'put',
        body: JSON.stringify({
            darkMode,
        }),
    }),
};
ApiUtils.getMonitors().then(console.log);
ApiUtils.toggleDarkMode().then(console.log);
ApiUtils.toggleDarkMode(true).then(console.log);
//# sourceMappingURL=renderer.js.map