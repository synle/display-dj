import { screen } from 'electron';

const APP_WIDTH = 300;
const APP_HEIGHT_MIN = 50;

const PositionUtils = {
  updateTrayPosition: async (contentHeight, tray, mainWindow) => {
    if (!tray) {
      throw `tray and mainWindow not ready yet`;
    }

    const width = APP_WIDTH;
    const height = Math.max(contentHeight, APP_HEIGHT_MIN);

    const trayBound = tray.getBounds();

    var mainScreen = screen.getPrimaryDisplay();
    const mainScreenSize = mainScreen.size;
    let x = trayBound.x;
    let y = trayBound.y;

    const xOffset = 50;
    const yOffset = 0;

    if (y > mainScreenSize.height / 2) {
      // bottom
      y = Math.floor(trayBound.y - height - yOffset);
    } else {
      // top
      y = Math.floor(trayBound.y + yOffset);
    }
    if (x > mainScreenSize.width / 2) {
      // right
      x = Math.floor(trayBound.x - width + xOffset);
    } else {
      // left
      x = Math.floor(trayBound.x + xOffset);
    }

    // TODO: need to adjust for left and right vertical taskbar

    mainWindow.setPosition(x, y);
    mainWindow.setSize(width, height);
  },
};

export default PositionUtils;
