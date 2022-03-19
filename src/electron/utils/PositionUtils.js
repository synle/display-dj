import { screen } from 'electron';

const PositionUtils = {
  updateTrayPosition: async (contentHeight, tray, mainWindow) => {
    if (!tray) {
      throw `tray and mainWindow not ready yet`;
    }

    const width = 300;
    const height = contentHeight;

    if(height === undefined || height <= 100){
      throw `height is required and need to be at least 100px`;
    }

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
