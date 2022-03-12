import ApiUtils from './utils/ApiUtils';


// TODO

ApiUtils.getMonitors().then(console.log);
ApiUtils.toggleDarkMode().then(console.log);
ApiUtils.toggleDarkMode(true).then(console.log);
ApiUtils.updateMonitor({
  id: "\\\\?\\DISPLAY#VSCB73A#5&21f33940&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}",
  brightness: 0
})

ApiUtils.updateMonitor({
  id: "\\\\?\\DISPLAY#VSCB73A#5&23c70c64&0&UID257#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}",
  name: "monitor #2222"
})
