const brightness = require('brightness');

brightness.get().then((level) => console.log(level));
