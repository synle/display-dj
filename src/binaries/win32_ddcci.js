// NOTE: refer to this PR: https://github.com/synle/display-dj/pull/65
// why do we need to keep this as a separate script?
// the problem is node ddcci doesn't work for some devices
// when you disconnect and reconnect
//
// having it as a separate script will make sure the initiation
// of the ddcci works properly in windows
let ddcci;

const path = require('path');
const { spawn, exec } = require('child_process');

process.on('message', async function (msg) {
  try {
    // first attempt to find ddcci as part of the dev mode (pure third party require)
    ddcci = require('@hensm/ddcci');
    if (!ddcci) {
      throw 'Cannot find local node module for ddcci';
    }
  } catch (err) {
    // otherwise, will look into it from the msg[0] = process['resourcesPath']
    ddcci = require(path.join(msg[0], 'node_modules/@hensm/ddcci'));
  }

  const command = msg[1];
  const targetMonitorId = msg[2];
  const newBrightness = parseInt(msg[3]);

  try {
    let res;
    switch (command) {
      case 'setBrightness':
        res = await _setBrightness(targetMonitorId, newBrightness);
        break;

      case 'getBrightness':
        res = await _getBrightness(targetMonitorId);
        break;

      case 'getMonitorList':
        res = await _getMonitorList();
        break;

      case 'customScript':
        const shellToRun = msg[2];
        res = await new Promise((resolve, reject) => {
          const child = spawn('powershell.exe', ['-Command', shellToRun]);

          let data = '';
          child.stdout.on('data', function (msg) {
            data += msg.toString();
          });

          child.on('exit', function (exitCode) {
            if (parseInt(exitCode) !== 0) {
              reject(exitCode);
            } else {
              resolve(data);
            }
          });
        });
        break;

      default:
        throw `Not supported command - ${command}`;
        break;
    }
    process.send({ success: true, command, data: res });
  } catch (error) {
    process.send({ success: false, command, error: error.toString() });
  }
  process.exit();
});

function _setBrightness(targetMonitorId, newBrightness) {
  if (isNaN(newBrightness) || newBrightness < 0 || newBrightness > 100) {
    throw 'newBrightness needs to be a number between 0 and 100';
  }

  for (const monitorId of ddcci.getMonitorList()) {
    if (monitorId === targetMonitorId) {
      try {
        return ddcci.setBrightness(monitorId, newBrightness);
      } catch (err) {
        throw err;
      }
    }
  }

  throw `targetMonitorId (${targetMonitorId}) not found`;
}

function _getBrightness(targetMonitorId) {
  return ddcci.getBrightness(targetMonitorId);
}

function _getMonitorList() {
  return ddcci.getMonitorList();
}
