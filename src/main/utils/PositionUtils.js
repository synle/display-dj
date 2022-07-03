import { screen } from 'electron';

const APP_WIDTH = 300;
const APP_HEIGHT_MIN = 50;

const PositionUtils = {
  updateTrayPosition: async (mainWindow, contentHeight) => {
    const width = APP_WIDTH;
    const height = Math.max(contentHeight, APP_HEIGHT_MIN);

    const mainScreen = screen.getPrimaryDisplay();
    const mainScreenSize = mainScreen.size;

    let { x, y } = global.trayPos || mainWindow.getBounds();

    const xOffset = 50;
    if (x > mainScreenSize.width / 2) {
      // right
      x = Math.floor(x - width + xOffset);
    } else {
      // left
      x = Math.floor(x + xOffset);
    }

    const yOffset = 0;
    if (y > mainScreenSize.height / 2) {
      // bottom
      y = Math.floor(y - height - yOffset);
    } else {
      // top
      y = Math.floor(y + yOffset);
    }

    // TODO: need to adjust for left and right vertical taskbar

    mainWindow.setPosition(x, y);
    mainWindow.setSize(width, height);
  },
};

export default PositionUtils;
