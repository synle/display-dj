const ddcci = require("@hensm/ddcci");

console.log('Child Process Spawned')

process.on('message', function(msg) {
  console.log('Child received message', msg);

  const monitorIdToChange = msg[0];
  const newBrightness = parseInt(msg[1]);

  _changeBrightness(monitorIdToChange, newBrightness);
  process.send('ChildMessage');
});

function _changeBrightness(){
  console.log('monitorIdToChange', monitorIdToChange)
  console.log('newBrightness', newBrightness)

  if(isNaN(newBrightness) || newBrightness < 0 || newBrightness > 100){
    console.log('No data', process.argv)
    process.exit(1);
  }

  for (const monitorId of ddcci.getMonitorList()) {
    console.log('monitorId', monitorId)
    if(monitorId === monitorIdToChange){
      try{
        ddcci.setBrightness(monitorId, newBrightness);
        console.log('Successfully set brightness for monitor', monitorIdToChange, newBrightness);
        process.exit(0);
      } catch(err){
        console.log('Failed to set brightness for monitor', monitorIdToChange, newBrightness, err);
        process.exit(1);
      }

    }
  }

  console.log('Monitor ID not found');
  process.exit(1);
}

