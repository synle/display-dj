import ReactDOM from 'react-dom';
import React, {useState, useEffect} from 'react';
import { QueryClient, QueryClientProvider, useQuery, useMutation, useQueryClient } from 'react-query';
import ApiUtils from './utils/ApiUtils';
import "./index.scss";


// // TODO
// ApiUtils.getMonitors().then(console.log);
// ApiUtils.toggleDarkMode().then(console.log);
// ApiUtils.toggleDarkMode(true).then(console.log);
// ApiUtils.updateMonitor({
//   id: '\\\\?\\DISPLAY#VSCB73A#5&21f33940&0&UID4352#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}',
//   brightness: 100,
// });

// ApiUtils.updateMonitor({
//   id: '\\\\?\\DISPLAY#VSCB73A#5&23c70c64&0&UID257#{e6f07b5f-ee97-4a90-b076-33f57bf4eaa7}',
//   name: 'monitor #2222',
// });



// TODO: extract these things
const queryClient = new QueryClient()

// react components
function Home(props) {
  const {isLoading, data: monitors} = useMonitors();

  if(isLoading){
    return <>Loading...</>
  }

  if(!monitors || monitors.length === 0){
    // TODO: add message for no data state
    return null;
  }

  return <>
    <header><h1>Display-DJ</h1></header>
      <MonitorBrightnessSettingForm monitors={monitors} />
      <AllMonitorBrightnessSettings monitors={monitors} />
      <DarkModeSettingForm />
  </>;
};

function DarkModeSettingForm(props){
  const [darkMode, setDarkMode] = useState(false);
  const {mutateAsync: toggleDarkMode} = useToggleDarkMode();

  const onToggleDarkMode = () => {
    const newDarkMode = !darkMode;
    setDarkMode(newDarkMode);
    toggleDarkMode(newDarkMode);
  }

  return <div className='field' title='Monitor Name'>
      <div className='field_value'>
        <button onClick={onToggleDarkMode}>{darkMode ? 'Turn Dark Mode Off': 'Turn Dark Mode On'}</button>
      </div>
  </div>
}

function MonitorBrightnessSettingForm(props){
  const {monitors} = props;
  return monitors.map((monitor, idx) => <MonitorBrightnessSetting key={monitor.id} monitor={monitor} idx={idx + 1} /> );
}

function MonitorBrightnessSetting(props){
  const [editName, setEditName] = useState(false);
  const {monitor } = props;
  const {mutateAsync: updateMonitor} = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = (key, value) => {
    monitor[key] = value;
    updateMonitor(monitor);
  }

  const onNameBlur = () => {
    monitor.name = monitor.name.trim();

    if(!monitor.name){
      monitor.name = `Monitor #${props.idx}`
    }

    setEditName(false);
    updateMonitor(monitor);
  }

  return <>
    <div className='field' title='Monitor Name'>
      {
        editName ? <input className='field__value' value={monitor.name} placeholder='name' autoFocus={true} onInput={(e) => onChange('name', e.target.value)} onBlur={onNameBlur} />
        :<div className='field__value field__value-readonly field__value-toggle' onClick={() => setEditName(true)}>{monitor.name}</div>
      }
    </div>
    <div className='field' title='Monitor Brightness'>
      <input className='field__value' type='range' min='0' max='100' step='5' value={monitor.brightness} placeholder='brightness' onInput={(e) => onChange('brightness', parseInt(e.target.value) || 0)} />
    </div>
  </>
}

function AllMonitorBrightnessSettings(props){
  const [allBrightness, setAllBrightness] = useState(50);
  const {monitors } = props;
  const {mutateAsync: updateMonitor} = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = async (value) => {
    setAllBrightness(value);

    for(const monitor of monitors){
      monitor.brightness = value;
      await updateMonitor(monitor);
    }

    queryClient.invalidateQueries(QUERY_KEY_MONITORS);
  }

  return <>
      <div className='field' title='Monitor Name'>
        <div className='field__value field__value-readonly'>All Monitors</div>
      </div>
      <div className='field'>
        <input className='field__value' type='range' min='0' max='100' step='5' value={allBrightness} placeholder='brightness' onInput={(e) => onChange(parseInt(e.target.value) || 0)} />
      </div>
  </>
}


// react query store
const QUERY_KEY_MONITORS = 'monitors';
function useMonitors(){
  return useQuery(QUERY_KEY_MONITORS, ApiUtils.getMonitors);
}

function useUpdateMonitor(){
  return useMutation(ApiUtils.updateMonitor);
}

function useToggleDarkMode(){
  return useMutation(ApiUtils.toggleDarkMode);
}


ReactDOM.render(<QueryClientProvider client={queryClient}><Home /></QueryClientProvider>, document.querySelector('#root'));
