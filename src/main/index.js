import AutoLaunch from 'auto-launch';
import { app, BrowserWindow, dialog, globalShortcut, ipcMain, Menu, nativeTheme, shell, Tray } from 'electron';
import { EventEmitter } from 'events';
import path from 'path';
import { matchPath } from 'react-router-dom';
import DisplayUtils from 'src/main/utils/DisplayUtils';
import { getEndpointHandlers, setUpDataEndpoints } from 'src/main/utils/Endpoints';
import { showNotification } from 'src/main/utils/NotificationUtils';
import PositionUtils from 'src/main/utils/PositionUtils';
import PreferenceUtils from 'src/main/utils/PreferenceUtils';
import { getJSON } from 'src/main/utils/RestUtils';
import SoundUtils from 'src/main/utils/SoundUtils';
import { LOG_FILE_PATH, MONITOR_CONFIG_FILE_PATH, PREFERENCE_FILE_PATH } from 'src/main/utils/StorageUtils';
import 'src/main/utils/LogUtils';
let mainWindow;

const appBaseDir = __dirname;

const DARK_ICON = path.join(appBaseDir, 'icon-dark.png');

const LIGHT_ICON = path.join(appBaseDir, 'icon-light.png');

const MAC_DOWNLOAD_LINK = `https://github.com/synle/display-dj/releases/latest/download/display-dj-darwin.dmg`;

const WINDOWS_DOWNLOAD_LINK = `https://github.com/synle/display-dj/releases/latest/download/display-dj-setup.exe`;

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

  let contextMenu = await _getContextMenu();
  tray.on('right-click', async function (event) {
    // when clicked on to open menu or context menu
    // we will reset keybindings
    setUpShortcuts();

    tray.popUpContextMenu(contextMenu);
    mainWindow.hide();

    contextMenu = await _getContextMenu();
  });
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

      if(keyBinding.notification){
        showNotification(keyBinding.notification);
      }
    });
  });

  if (!keybindingSuccess.every((success) => success)) {
    console.error(
      'globalShortcut keybinding failed',
      keyBindings.map((keyBinding, idx) => `${keyBinding.key} = ${keybindingSuccess[idx]}`),
    );
  } else {
    console.info('globalShortcut keybinding success');
  }
}

function setupAutolaunch() {
  if (process.env.APP_MODE !== 'production') {
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
    return AppEventEmitter.on(eventName, callbackFunc);
  };

  global.subscribeAppEvent(async (data) => {
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
        default:
          delta = 0;
          allMonitorBrightness = parseInt(command.replace('command/changeBrightness/', ''));
          break;
      }

      if(!isNaN(allMonitorBrightness) && allMonitorBrightness >= 0 && allMonitorBrightness <= 100){
        await DisplayUtils.batchUpdateBrightness(allMonitorBrightness, delta);
      } else {
        console.trace(`changeVolume failed due to invalid volume`, allMonitorBrightness, delta);
      }

      _sendRefetchEventToFrontEnd();
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
      _sendRefetchEventToFrontEnd();
      return;
    }

    if (command.includes(`command/changeVolume`)) {
      const volume = parseInt(command.replace('command/changeVolume/', ''));

      if(!isNaN(volume) && volume >= 0 && volume <= 100){
        const promises = [];
        promises.push(SoundUtils.setMuted(volume === 0));
        promises.push(SoundUtils.setVolume(volume));
        await Promise.all(promises)
        _sendRefetchEventToFrontEnd();
      } else {
        console.trace(`changeVolume failed due to invalid volume`, volume);
      }

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
          locationToUse = 'https://synle.github.io/display-dj/';
          break;
        case 'command/openExternal/link/downloadNewVersion':
          locationToUse = process.platform === 'darwin' ? MAC_DOWNLOAD_LINK : WINDOWS_DOWNLOAD_LINK;
          break;
      }

      shell.openExternal(`${protocol}${locationToUse}`);
      return;
    }

    if (command.includes(`command/reset`)) {
      await DisplayUtils.reset();
      await setUpShortcuts(); // call this to reset keyboard shortcut
      _sendRefetchEventToFrontEnd();
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

async function _getLatestAppVersion(){
  try{
    const {version} = await getJSON(`https://synle.github.io/display-dj/release.json`);
    return version;
  } catch(err){
    return '';
  }
}

async function _getContextMenu(){
  const brightnessPresets = await PreferenceUtils.getBrightnessPresets();
  const volumePresets = await PreferenceUtils.getVolumePresets();
  const latestAppVersion = await _getLatestAppVersion();

  const contextMenu = Menu.buildFromTemplate([
    {
      label: `Use Light Mode`,
      click: async () => {
        global.emitAppEvent({ command: 'command/changeDarkMode/light' })
        showNotification(`Turn on Light Mode`);
      },
    },
    {
      label: `Use Dark Mode`,
      click: async () => {
        global.emitAppEvent({ command: 'command/changeDarkMode/dark' })
        showNotification(`Turn on Dark Mode`);
      },
    },
    {
      type: 'separator',
    },
    ...brightnessPresets.map(brightnessPreset => ({
      label: `Change brightness to ${brightnessPreset.level}%`,
      click: async () => {
        global.emitAppEvent({ command: `command/changeBrightness/${brightnessPreset.level}` })
        showNotification(`Brightness of all monitors changed to ${brightnessPreset.level}%`);
      },
    })),
    {
      type: 'separator',
    },
    ...volumePresets.map(volumePreset => ({
      label: `Change volume to ${volumePreset.level}%`,
      click: async () => {
        global.emitAppEvent({ command: `command/changeVolume/${volumePreset.level}` })
        showNotification(`Volume changed to ${volumePreset.level}%`);
      },
    })),
    {
      type: 'separator',
    },
    {
      label: `Open Monitor Configs`,
      click: async () =>
        global.emitAppEvent({ command: 'command/openExternal/file/monitorConfigs' }),
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
      label: `New Version Available (${latestAppVersion})`,
      click: async () => global.emitAppEvent({ command: 'command/openExternal/link/downloadNewVersion' }),
      visible: await latestAppVersion !== process.env.APP_VERSION
    },
    {
      type: 'separator',
    },
    {
      label: `Reset`,
      click: async () => {
        try {
          const buttons = ['Yes', 'No'];
          const responseResult = await dialog.showMessageBox({
            buttons,
            message: 'Do you want to reset all monitor configs and preferences?',
          });

          if (responseResult.response === buttons.indexOf('Yes')) {
            console.trace('Continue with Reset application configs and preferences');
            global.emitAppEvent({ command: 'command/reset' });
            return;
          }
        } catch (err) {}
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

  return contextMenu;
}

/**
 * @return None - Hide the dock icon for Mac...
 */
async function setupDockIcon(){
  switch(process.platform){
    case 'darwin':
      try{
        app.dock.hide();
        console.debug('Hide dock icon success');
      } catch(err){
        console.error('Hide dock icon failed with error', err);
      }
      break;
  }
}

function _sendRefetchEventToFrontEnd(eventName = 'mainAppEvent/refetch', eventData = {}){
    if(mainWindow){
      mainWindow.webContents.send(eventName, eventData);
    }
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

// hide app doc icons (applicable for mac)
setupDockIcon();

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
