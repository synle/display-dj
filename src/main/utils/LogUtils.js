import PreferenceUtils from 'src/main/utils/PreferenceUtils';
import StorageUtils, { LOG_FILE_PATH } from 'src/main/utils/StorageUtils';
function _appendLogToFile(...data) {
  const logLine = `[${new Date().toLocaleString()}] ${_serializeLogData(...data)}\n`;

  if (StorageUtils.size(LOG_FILE_PATH) > 100) {
    // if the file is greater threshold in MB, then do this as a write
    // to wipe out the old logs
    StorageUtils.write(LOG_FILE_PATH, logLine);
  } else {
    // otherwise append the log
    StorageUtils.append(LOG_FILE_PATH, logLine);
  }
}

function _serializeLogData(...data) {
  const res = [];

  for (const item of data) {
    if (Array.isArray(item)) {
      res.push(item.map(JSON.stringify).join(', '));
    } else if (item.constructor === Object) {
      res.push(JSON.stringify(item));
    } else {
      res.push(item);
    }
  }

  return res.join('  ');
}

// colors and stylings for console logs
String.prototype.blue = function () {
  return `\x1b[36m${this}\x1b[0m`;
};

String.prototype.yellow = function () {
  return `\x1b[33m${this}\x1b[0m`;
};

String.prototype.green = function () {
  return `\x1b[32m${this}\x1b[0m`;
};

String.prototype.red = function () {
  return `\x1b[31m${this}\x1b[0m`;
};

async function _initializeLogUtils(){
  const preferences = await PreferenceUtils.get();
  if(preferences.logging === true){
    // enable logging
    const origConsole = console.log;
    console.log = (...data) => {
      origConsole('[LOG]'.green(), ...data);

      _appendLogToFile(`[LOG]`, ...data);
    };

    console.info = (...data) => {
      origConsole('[INFO]'.green(), ...data);

      _appendLogToFile(`[INFO]`, ...data);
    };
    console.error = (...data) => {
      origConsole('[ERROR]'.red(), ...data);

      _appendLogToFile(`[ERROR]`, ...data);
    };
    console.debug = (...data) => {
      origConsole('[DEBUG]'.yellow(), ...data);

      _appendLogToFile(`[DEBUG]`, ...data);
    };
    console.trace = (...data) => {
      origConsole('[TRACE]'.blue(), ...data);

      _appendLogToFile(`[TRACE]`, ...data);
    };
  } else {
    // disable logging
    const origConsole = console.log;
    console.log = () => {}
    console.info = () => {}
    console.error = () => {}
    console.debug = () => {}
    console.trace = () => {}
    console.verbose = origConsole
  }
}

_initializeLogUtils();
