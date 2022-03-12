import { BrowserWindow, Menu, Tray, app, ipcMain, systemPreferences } from 'electron';
import { matchPath } from 'react-router-dom';
import * as path from 'path';
import { setUpDataEndpoints, getEndpointHandlers } from './utils/Endpoints';

let mainWindow;

function createWindow() {
  // Create the browser window.
  mainWindow = new BrowserWindow({
    height: 500,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
    },
    width: 400,
    show: false,
    autoHideMenuBar: true,
    icon: path.join(__dirname, '..', 'icon.ico'),
  });

  mainWindow.on('minimize', function (event) {
    event.preventDefault();
    mainWindow.hide();
  });

  mainWindow.on('close', function (event) {
    event.preventDefault();
    mainWindow.hide();

    return false;
  });

  // and load the index.html of the app.
  mainWindow.loadFile(path.join(__dirname, '..', 'index.html'));

  // Open the DevTools.
  // mainWindow.webContents.openDevTools({ mode: 'detach' });
}

function createTray() {
  //https://livebook.manning.com/book/electron-in-action/chapter-9/1
  // systemPreferences.isDarkMode()
  const tray = new Tray(path.join(__dirname, '..', 'icon.png'));

  /*
  [Arguments] {
    '0': {
      shiftKey: false,
      ctrlKey: false,
      altKey: false,
      metaKey: false,
      triggeredByAccelerator: true
    },
    '1': { x: 1793, y: 1104, width: 25, height: 48 },
    '2': { x: 1800, y: 1129 }
  }
  */

  tray.on('click', function (event, iconPos, mousePos) {
    console.log(this);
    console.log(arguments);
    if (mainWindow.isVisible()) {
      mainWindow.hide();
    } else {
      let x = Math.floor(iconPos.x - mainWindow.getBounds().width + 50);
      let y = Math.floor(iconPos.y - mainWindow.getBounds().height);
      mainWindow.setPosition(x, y);
      mainWindow.show();
    }
  });

  const menu = Menu.buildFromTemplate([
    {
      label: 'Quit',
      click() {
        app.quit();
      },
    },
  ]);

  tray.setToolTip('Clipmaster');
  tray.setContextMenu(menu);
}

// This method will be called when Electron has finished
// initialization and is ready to create browser windows.
// Some APIs can only be used after this event occurs.
app.on('ready', () => {
  setUpDataEndpoints();
  createWindow();
  createTray();

  app.on('activate', function () {
    // On macOS it's common to re-create a window in the app when the
    // dock icon is clicked and there are no other windows open.
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
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
