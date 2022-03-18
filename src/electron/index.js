import AutoLaunch from 'auto-launch';
import electron, { Menu, Tray, app, globalShortcut, ipcMain, nativeTheme, shell } from 'electron';
import { matchPath } from 'react-router-dom';
import path from 'path';
import { setUpDataEndpoints, getEndpointHandlers } from './utils/Endpoints';
import DisplayUtils from './utils/DisplayUtils';
import { MONITOR_CONFIG_FILE_DIR } from '../constants';

const { menubar } = require('menubar');

let mainWindow;

const appBaseDir = __dirname;

const DARK_ICON = path.join(appBaseDir, 'icon-dark.png');

const LIGHT_ICON = path.join(appBaseDir, 'icon-light.png');

console.error = console.log.bind(null, 'ERROR');

async function setUpShortcuts() {
  // TODO: move this into a config
  const keyBrightnessDown = `Shift+F1`;
  const keyBrightnessUp = `Shift+F2`;
  const delta = 50;
  let isChangingAllMonitorBrightness = false;
  let allMonitorBrightness = await DisplayUtils.getAllMonitorsBrightness();
  const keybindingSuccess = [];
  keybindingSuccess.push(
    globalShortcut.register(keyBrightnessDown, async () => {
      if (isChangingAllMonitorBrightness) {
        return;
      }
      isChangingAllMonitorBrightness = true;
      try {
        allMonitorBrightness = await DisplayUtils.updateAllBrightness(
          allMonitorBrightness,
          -1 * delta,
        );
      } catch (err) {}
      isChangingAllMonitorBrightness = false;
    }),
  );

  keybindingSuccess.push(
    globalShortcut.register(keyBrightnessUp, async () => {
      if (isChangingAllMonitorBrightness) {
        return;
      }

      isChangingAllMonitorBrightness = true;
      try {
        allMonitorBrightness = await DisplayUtils.updateAllBrightness(allMonitorBrightness, delta);
      } catch (err) {}
      isChangingAllMonitorBrightness = false;
    }),
  );

  if (!keybindingSuccess.every((success) => success)) {
    console.log('>> globalShortcut keybinding failed');
  } else {
    console.log('>> globalShortcut keybinding success');
  }
}
function setupAutolaunch() {
  if (process.env.APPLICATION_MODE !== 'production') {
    return;
  }

  let autoLaunch = new AutoLaunch({
    name: 'display-dj',
    path: app.getPath('exe'),
  });
  autoLaunch.isEnabled().then((isEnabled) => {
    if (!isEnabled) autoLaunch.enable();
  });
}

async function setupMenuBar() {
  const mb = menubar({
    browserWindow: {
      webPreferences: {
        nodeIntegration: true,
        contextIsolation: false,
      },
      frame: false,
      autoHideMenuBar: true,
      width: 300,
      height: 250,
      alwaysOnTop: true,
    },
    icon: (await DisplayUtils.getDarkMode()) === true ? DARK_ICON : LIGHT_ICON,
    tooltip: `display-dj (by Sy Le)`,
    preloadWindow: true,
  });

  mb.on('ready', () => {
    const { tray } = mb;


    // setting up tray icon
    nativeTheme.on('updated', () => {
      tray.setImage(_getTrayIcon());
    });

    const menu = Menu.buildFromTemplate([
      {
        label: 'Change brightness to 0%',
        click: () => DisplayUtils.updateAllBrightness(0),
      },
      {
        label: 'Change brightness to 50%',
        click: () => DisplayUtils.updateAllBrightness(50),
      },
      {
        label: 'Change brightness to 100%',
        click: () => DisplayUtils.updateAllBrightness(100),
      },
      {
        type: 'separator',
      },
      {
        label: 'Open Configs',
        click: () => {
          try {
            console.log(`file://${MONITOR_CONFIG_FILE_DIR}`);
            shell.openExternal(`file://${MONITOR_CONFIG_FILE_DIR}`);
          } catch (err) {}
        },
      },
      {
        type: 'separator',
      },
      {
        label: 'File a bug',
        click: async () => {
          await shell.openExternal('https://github.com/synle/display-dj/issues/new');
        },
      },
      {
        label: 'About display-dj',
        click: async () => {
          await shell.openExternal('https://github.com/synle/display-dj');
        },
      },
      {
        type: 'separator',
      },
      {
        label: 'Exit',
        click: () => app.quit(),
      },
    ]);

    tray.setContextMenu(menu);
  });
}

function _getTrayIcon() {
  return nativeTheme.shouldUseDarkColors ? DARK_ICON : LIGHT_ICON;
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', () => {
  setUpDataEndpoints();
  setupMenuBar();
  setUpShortcuts();
  setupAutolaunch();
});

// Quit when all windows are closed, except on macOS. There, it's common
// for applications and their menu bar to stay active until the user quits
// explicitly with Cmd + Q.
app.on('window-all-closed', () => app.quit());

// In this file you can include the rest of your app"s specific main process
// code. You can also put them in separate files and require them here.
// this is the event listener that will respond when we will request it in the web page
const _apiCache = {};
ipcMain.on('mainAppEvent/fetch', async (event, data) => {
  const { requestId, url, options } = data;
  const responseId = `server response ${Date.now()}`;

  const method = (options.method || 'get').toLowerCase();

  const sessionId = options.headers['api-call-session-id'] || 'display-dj-default-session';

  let body = {};
  try {
    body = JSON.parse(options.body);
  } catch (err) {}

  console.log('>> Request', method, url, sessionId, body);
  let matchedUrlObject;
  const matchCurrentUrlAgainst = (matchAgainstUrl) => {
    try {
      return matchPath(matchAgainstUrl, url);
    } catch (err) {
      return undefined;
    }
  };

  try {
    let returnedResponseHeaders = []; // array of [key, value]
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

    // polyfill for the express server interface
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

    const endpoints = getEndpointHandlers();

    for (const endpoint of endpoints) {
      const [targetMethod, targetUrl, targetHandler] = endpoint;
      const matchedUrlObject = matchCurrentUrlAgainst(targetUrl);
      if (targetMethod === method && matchedUrlObject) {
        const apiCache = {
          get(key) {
            try {
              //@ts-ignore
              return _apiCache[sessionId][key];
            } catch (err) {
              return undefined;
            }
          },
          set(key, value) {
            try {
              //@ts-ignore
              _apiCache[sessionId] = _apiCache[sessionId] || {};

              //@ts-ignore
              _apiCache[sessionId][key] = value;
            } catch (err) {}
          },
          json() {
            return JSON.stringify(_apiCache);
          },
        };

        const req = {
          params: matchedUrlObject.params,
          body: body,
          headers: {
            ['api-call-session-id']: sessionId,
          },
        };

        return targetHandler(req, res, apiCache);
      }
    }

    // not found, then return 404
    sendResponse('Resource Not Found...', 500);
  } catch (err) {
    console.log('error', err);
  }
});
