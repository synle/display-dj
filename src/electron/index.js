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
  dialog,
} from 'electron';
import DisplayUtils from 'src/electron/utils/DisplayUtils';
import { getEndpointHandlers, setUpDataEndpoints } from 'src/electron/utils/Endpoints';
import PositionUtils from 'src/electron/utils/PositionUtils';
import PreferenceUtils from 'src/electron/utils/PreferenceUtils';
import { matchPath } from 'react-router-dom';
import { EventEmitter } from 'events';
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
    skipTaskbar: true,
  });

  mainWindow.on('minimize', (e) => {
    e.preventDefault();
    mainWindow.hide();
  });

  mainWindow.on('blur', (e) => {
    e.preventDefault();
    mainWindow.hide();
  });

  // and load the index.html of the app.
  mainWindow.loadFile(path.join(appBaseDir, 'index.html'));

  // Open the DevTools.
  // mainWindow.webContents.openDevTools({ mode: 'detach' });
  global.mainWindow = mainWindow;
}

async function createTray() {
  nativeTheme.on('updated', () => {
    tray.setImage(_getTrayIcon());
  });

  tray.on('click', async (event, trayBounds, mousePos) => {
    // when clicked on to open menu or context menu
    // we will reset keybindings
    setUpShortcuts();

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
    // when clicked on to open menu or context menu
    // we will reset keybindings
    setUpShortcuts();

    tray.popUpContextMenu(contextMenu);
    mainWindow.hide();
  });

  const contextMenu = Menu.buildFromTemplate([
    {
      label: `Change brightness to 0%`,
      click: async () => global.emitAppEvent({ command: 'command/changeBrightness/0' }),
    },
    {
      label: `Change brightness to 50%`,
      click: async () => global.emitAppEvent({ command: 'command/changeBrightness/50' }),
    },
    {
      label: `Change brightness to 100%`,
      click: async () => global.emitAppEvent({ command: 'command/changeBrightness/100' }),
    },
    {
      type: 'separator',
    },
    {
      label: `Use Light Mode`,
      click: async () => global.emitAppEvent({ command: 'command/changeDarkMode/light' }),
    },
    {
      label: `Use Dark Mode`,
      click: async () => global.emitAppEvent({ command: 'command/changeDarkMode/dark' }),
    },
    {
      type: 'separator',
    },
    {
      label: `Open Monitor Configs`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/file/monitorConfigs' }),
    },
    {
      label: `Open App Preferences`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/file/preferences' }),
    },
    {
      label: `Open Dev Logs`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/file/devLogs' }),
    },
    {
      type: 'separator',
    },
    {
      label: `File a bug`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/link/bugReport' }),
    },
    {
      label: `About display-dj (${process.env.APP_VERSION})`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/link/aboutUs' }),
    },
    {
      type: 'separator',
    },
    {
      label: `Reset`,
      click: async () => {
        try{
          const buttons = ["Yes","No"]
          const responseResult = await dialog.showMessageBox({
           buttons,
           message: "Do you want to reset all monitor configs and preferences?"
          })

          if(responseResult.response === buttons.indexOf('Yes')){
            console.trace('Continue with Reset application configs and preferences');
            global.emitAppEvent({ command: 'command/reset' })
            return;
          }
        } catch(err){}
        console.trace('Skip reset application configs and preferences');
      },
    },
    {
      type: 'separator',
    },
    {
      label: `Exit`,
      click: async () => {
        app.quit();
        process.exit();
      },
    },
  ]);
}

async function setUpShortcuts() {
  // reset keybindings first
  globalShortcut.unregisterAll();

  const keyBindings = await PreferenceUtils.getKeybindings();

  const keybindingSuccess = keyBindings.map((keyBinding) => {
    return globalShortcut.register(keyBinding.key, async () => {
      const commands = [].concat(keyBinding.command);
      for (const command of commands) {
        global.emitAppEvent({ command });
      }
    });
  });

  if (!keybindingSuccess.every((success) => success)) {
    console.error(
      'globalShortcut keybinding failed',
      keyBindings.map(
        (keyBinding, idx) => `${keyBinding.key} = ${keybindingSuccess[idx]}`,
      ),
    );
  } else {
    console.info('globalShortcut keybinding success');
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

async function setupCommandChannel() {
  let allMonitorBrightness = await DisplayUtils.getAllMonitorsBrightness();
  let darkModeToUse = (await DisplayUtils.getDarkMode()) === true;
  const preferences = await PreferenceUtils.get();

  const AppEventEmitter = new EventEmitter();

  const EVENT_KEY_MISSION_CONTROL = 'GlobalAppEvent';

  global.emitAppEvent = (data, eventName = EVENT_KEY_MISSION_CONTROL) => {
    AppEventEmitter.emit(eventName, data);
  };

  global.subscribeAppEvent = (callbackFunc, eventName = EVENT_KEY_MISSION_CONTROL) => {
    // TODO: not used right now
  };
  AppEventEmitter.on(EVENT_KEY_MISSION_CONTROL, async (data) => {
    const { command } = data;

    if (command.includes(`command/changeBrightness`)) {
      // these commands are change brightness
      let delta = preferences.brightnessDelta;

      switch (command) {
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
        case 'command/changeBrightness/10':
          delta = 0;
          allMonitorBrightness = 10;
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

      await DisplayUtils.batchUpdateBrightness(allMonitorBrightness, delta);
      return;
    }

    if (command.includes(`command/changeDarkMode`)) {
      switch (command) {
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
      return;
    }

    if (command.includes(`command/openExternal`)) {
      let locationToUse;
      let protocol = command.includes(`command/openExternal/file`) ? 'file://' : '';

      switch (command) {
        case 'command/openExternal/file/monitorConfigs':
          locationToUse = MONITOR_CONFIG_FILE_PATH;
          break;
        case 'command/openExternal/file/preferences':
          locationToUse = PREFERENCE_FILE_PATH;
          break;
        case 'command/openExternal/file/devLogs':
          locationToUse = LOG_FILE_PATH;
          break;
        case 'command/openExternal/link/bugReport':
          locationToUse = 'https://github.com/synle/display-dj/issues/new';
          break;
        case 'command/openExternal/link/aboutUs':
          locationToUse = 'https://github.com/synle/display-dj';
          break;
      }

      shell.openExternal(`${protocol}${locationToUse}`);
      return;
    }

    if (command.includes(`command/reset`)) {
      await DisplayUtils.reset();
      await setUpShortcuts(); // call this to reset keyboard shortcut
      return;
    }
  });
}

/**
 * This routine runs once on load to make sure all the brightness are in sync with your configs
 */
async function synchronizeBrightness() {
  const monitors = await DisplayUtils.getMonitorsFromStorage();

  const promises = [];
  for (const monitor of monitors) {
    promises.push(
      new Promise(async (resolve) => {
        try {
          await DisplayUtils.updateMonitorBrightness(monitor.id, monitor.brightness);
          console.trace('Sync monitor brightness on load', monitor.name, monitor.brightness);
        } catch (err) {
          console.error('Failed to sync monitor brightness on load', monitor.name, monitor.id);
        }
        resolve();
      }),
    );
  }

  await Promise.all(promises);
}

function _getTrayIcon() {
  return nativeTheme.shouldUseDarkColors ? DARK_ICON : LIGHT_ICON;
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', async () => {
  // show the tray as soon as possible
  global.tray = new Tray((await DisplayUtils.getDarkMode()) === true ? DARK_ICON : LIGHT_ICON);

  await setupCommandChannel();
  await setUpDataEndpoints();
  await synchronizeBrightness();
  await createWindow();
  await createTray();
  await setupAutolaunch();
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
