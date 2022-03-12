const DisplayUtils = require('./dist/utils/DisplayUtils').default;

async function doWork() {
  const monitors = await DisplayUtils.getMonitors();
  console.log(monitors);

  for (const monitor of monitors) {
    await DisplayUtils.updateMonitor({
      ...monitor,
      brightness: 100,
    });
  }
}

doWork();
