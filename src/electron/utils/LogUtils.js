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

console.info = console.log.bind(null, '[INFO]'.green());
console.error = console.log.bind(null, '[ERROR]'.red());
console.debug = console.log.bind(null, '[DEBUG]'.yellow());
console.trace = console.log.bind(null, '[TRACE]'.blue());
