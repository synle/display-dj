var osascript = require('node-osascript');
 
const command = `set Volume 10`
osascript.execute(command, function(err, result, raw){
  if (err) return console.error(err)
  console.log(result, raw)
});

