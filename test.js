// const DisplayUtils = require('./build/utils/DisplayUtils').default;
const brightness = require('brightness');

async function doWork() {
  // const monitors = await DisplayUtils.getMonitors();
  // console.log(monitors);

  // let i = 0;
  // for (const monitor of monitors) {
  //   await DisplayUtils.updateMonitor({
  //     ...monitor,
  //     name: `Display #${++i}`,
  //     brightness: 100,
  //   });
  // }

  let newBrightness = 1;
  brightness.set(newBrightness);
}

doWork();
