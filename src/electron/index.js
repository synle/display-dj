import AutoLaunch from 'auto-launch';
import {
  BrowserWindow,
  Menu,
  Tray,
  app,
  globalShortcut,
  ipcMain,
  nativeTheme,
  shell,
} from 'electron';
import DisplayUtils from 'src/electron/utils/DisplayUtils';
import { getEndpointHandlers, setUpDataEndpoints } from 'src/electron/utils/Endpoints';
import PositionUtils from 'src/electron/utils/PositionUtils';
import PreferenceUtils from 'src/electron/utils/PreferenceUtils';
import { matchPath } from 'react-router-dom';
import path from 'path';
import StorageUtils, {
  MONITOR_CONFIG_FILE_PATH,
  PREFERENCE_FILE_PATH,
  LOG_FILE_PATH,
} from 'src/electron/utils/StorageUtils';
import 'src/electron/utils/LogUtils';
let mainWindow;

const appBaseDir = __dirname;

const DARK_ICON = path.join(appBaseDir, 'icon-dark.png');

const LIGHT_ICON = path.join(appBaseDir, 'icon-light.png');

function createWindow() {
  // Create the browser window.
  mainWindow = new BrowserWindow({
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
    },
    frame: false,
    show: false,
    autoHideMenuBar: true,
    alwaysOnTop: true,
    icon: path.join(appBaseDir, 'icon.ico'),
    width: 300,
    height: 200,
  });

  mainWindow.on('minimize', function (event) {
    event.preventDefault();
    mainWindow.hide();
  });

  // mainWindow.on('close', function (event) {
  //   event.preventDefault();
  //   mainWindow.hide();

  //   return false;
  // });

  mainWindow.on('blur', function (event) {
    event.preventDefault();
    mainWindow.hide();
  });

  // and load the index.html of the app.
  mainWindow.loadFile(path.join(appBaseDir, 'index.html'));

  // Open the DevTools.
  // mainWindow.webContents.openDevTools({ mode: 'detach' });
  global.mainWindow = mainWindow;
}

async function createTray() {
  let tray = new Tray((await DisplayUtils.getDarkMode()) === true ? DARK_ICON : LIGHT_ICON);
  global.tray = tray;

  tray.setToolTip(
    process.env.APPLICATION_MODE === 'dev'
      ? `display-dj (by Sy Le) (${process.env.APPLICATION_MODE})`
      : `display-dj (by Sy Le)`,
  );

  nativeTheme.on('updated', () => {
    tray.setImage(_getTrayIcon());
  });

  tray.on('click', async (event, trayBounds, mousePos) => {
    try {
      if (mainWindow.isVisible()) {
        mainWindow.hide();
      } else {
        mainWindow.show();

        const { width, height } = mainWindow.getBounds();

        global.trayPos = trayBounds;

        PositionUtils.updateTrayPosition(global.mainWindow, height);
      }
    } catch (err) {}
  });

  tray.on('right-click', function (event) {
    tray.popUpContextMenu(contextMenu);
    mainWindow.hide();
  });

  const contextMenu = Menu.buildFromTemplate([
    {
      label: 'Change brightness to 0%',
      click: () => DisplayUtils.batchUpdateBrightness(0),
    },
    {
      label: 'Change brightness to 50%',
      click: () => DisplayUtils.batchUpdateBrightness(50),
    },
    {
      label: 'Change brightness to 100%',
      click: () => DisplayUtils.batchUpdateBrightness(100),
    },
    {
      type: 'separator',
    },
    {
      label: 'Open Monitor Configs',
      click: () => {
        try {
          shell.openExternal(`file://${MONITOR_CONFIG_FILE_PATH}`);
        } catch (err) {}
      },
    },
    {
      label: 'Open App Preferences',
      click: () => {
        try {
          shell.openExternal(`file://${PREFERENCE_FILE_PATH}`);
        } catch (err) {}
      },
    },
    {
      label: 'Open Dev Logs',
      click: () => {
        try {
          shell.openExternal(`file://${LOG_FILE_PATH}`);
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
      click: () => {
        app.quit();
        process.exit();
      },
    },
  ]);
}

async function setUpShortcuts() {
  let allMonitorBrightness = await DisplayUtils.getAllMonitorsBrightness();
  let darkModeToUse = (await DisplayUtils.getDarkMode()) === true;

  const preferences = await PreferenceUtils.get();

  const keybindingSuccess = preferences.keyBindings.map((keyBinding) => {
    return globalShortcut.register(keyBinding.key, async () => {
      if (keyBinding.command.includes(`command/changeBrightness`)) {
        // these commands are change brightness
        let delta = preferences.brightnessDelta;

        switch (keyBinding.command) {
          case 'command/changeBrightness/down':
            delta = -1 * preferences.brightnessDelta;
            break;
          case 'command/changeBrightness/up':
            delta = preferences.brightnessDelta;
            break;
          case 'command/changeBrightness/0':
            delta = 0;
            allMonitorBrightness = 0;
            break;
          case 'command/changeBrightness/50':
            delta = 0;
            allMonitorBrightness = 50;
            break;
          case 'command/changeBrightness/100':
            delta = 0;
            allMonitorBrightness = 100;
            break;
        }

        allMonitorBrightness = await DisplayUtils.batchUpdateBrightness(
          allMonitorBrightness,
          delta,
        );
      } else if (keyBinding.command.includes(`command/changeDarkMode`)) {
        switch (keyBinding.command) {
          case 'command/changeDarkMode/toggle':
            darkModeToUse = (await DisplayUtils.getDarkMode()) === true;
            darkModeToUse = !darkModeToUse;
            break;
          case 'command/changeDarkMode/light':
            darkModeToUse = false;
            break;
          case 'command/changeDarkMode/dark':
            darkModeToUse = true;
            break;
        }

        await DisplayUtils.updateDarkMode(darkModeToUse);
      }
    });
  });

  if (!keybindingSuccess.every((success) => success)) {
    console.error(
      'globalShortcut keybinding failed',
      preferences.keyBindings.map(
        (keyBinding, idx) => `${keyBinding.key} = ${keybindingSuccess[idx]}`,
      ),
    );
  } else {
    console.info('globalShortcut keybinding success');
  }
}
function setupAutolaunch() {
  if (process.env.APPLICATION_MODE === 'dev') {
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

/**
 * This routine runs once on load to make sure all the brightness are in sync with your configs
 */
async function synchronizeBrightness(){
  const monitors = await DisplayUtils.getMonitorsFromStorage();

  for(const monitor of monitors){
    try{
      await DisplayUtils.updateMonitorBrightness(monitor.id, monitor.brightness);
      console.trace('Sync monitor brightness on load', monitor.name, monitor.brightness);
    } catch(err){
      console.error('Failed to sync monitor brightness on load', monitor.name, monitor.id);
    }
  }
}

function _getTrayIcon() {
  return nativeTheme.shouldUseDarkColors ? DARK_ICON : LIGHT_ICON;
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', async () => {
  await synchronizeBrightness();
  await createWindow();
  await createTray();
  await setupAutolaunch();
  await setUpDataEndpoints();
  await setUpShortcuts();
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

  console.trace('Request', method, url, sessionId, body);
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
      console.trace(
        'Response',
        status,
        method,
        url,
        body,
        '\n' + JSON.stringify(responseData, null, 2),
      );
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
    console.info('error', err);
  }
});
