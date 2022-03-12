const DisplayUtils = require('./dist/utils/DisplayUtils').default;

async function doWork() {
  const monitors = await DisplayUtils.getMonitors();
  console.log(monitors);

  let i = 0;
  for (const monitor of monitors) {
    await DisplayUtils.updateMonitor({
      ...monitor,
      name: `Display #${++i}`,
      brightness: 100,
    });
  }
}

doWork();
