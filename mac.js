const { exec } = require('child_process');
// [
//   ' CGDisplay 5C42D349-4EBD-DDE9-4526-94C54B3C2984 dispID(#418238087) (2560x1440 0°) 109.00 DPI',
//   ' CGDisplay 0FEF7274-2BC1-D7E7-C249-F8F65C82E015 dispID(#418238088) (2560x1440 0°) 109.00 DPI'
// ]
async function getMonitorList() {
  return new Promise((resolve, reject) => {
    const shellToRun = `ddcctl`;
    console.log(shellToRun);
    exec(shellToRun, (error, stdout, stderr) => {
      const monitors = (stdout || '')
        .split('\n')
        .filter((line) => line.indexOf('D:') === 0)
        .map((line, idx) => idx + 1);
      resolve(monitors);
    });
  });
}
async function updateMonitorBrightness(monitorId, newBrightness) {
  return new Promise((resolve, reject) => {
    const shellToRun = `ddcctl -d ${monitorId} -b ${newBrightness}`;
    console.log(shellToRun);
    exec(shellToRun, (error, stdout, stderr) => {
      if (error) {
        return reject(stderr);
      }
      resolve();
    });
  });
}
getMonitorList().then(async (monitorIds) => {
  console.log(monitorIds);
  for (const monitorId of monitorIds) {
    try {
      await updateMonitorBrightness(monitorId, 0);
    } catch (err) {
      console.log('Failed', err);
    }
  }
});

// process
//   .on("unhandledRejection", (reason, p) => {
//     console.error("Unhandled Rejection at Promise", reason, p);
//   })
//   .on("uncaughtException", (err) => {
//     console.error("Uncaught Exception thrown", err, err.stack);
//     process.exit(1);
//   });
