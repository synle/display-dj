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
  return <>
  <QueryClientProvider client={queryClient}>
    <MonitorBrightnessSettingForm />
    <DarkModeSettingForm />
    </QueryClientProvider>
  </>;
};

function DarkModeSettingForm(props){
  return <div>DarkModeSettingForm</div>
}

function MonitorBrightnessSettingForm(props){
  const {isLoading, data} = useMonitors();

  if(isLoading){
    return <>Loading...</>
  }

  if(!data || data.length === 0){
    // TODO: add message for no data state
    return null;
  }

  return data.map((monitor, idx) => <MonitorBrightnessSetting key={monitor.id} monitor={monitor} idx={idx + 1} /> );
}

function MonitorBrightnessSetting(props){
  const [editName, setEditName] = useState(true);
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

  return <div>
    <div className='field'>
      <label className='field__label'>Name:</label>
      {
        editName ? <input className='field__value' value={monitor.name} placeholder='name' autoFocus={true} onInput={(e) => onChange('name', e.target.value)} onBlur={onNameBlur} />
        :<div className='field__value field__value-readonly field__value-toggle' onClick={() => setEditName(true)}>{monitor.name}</div>
      }
    </div>
    <div className='field'>
      <label className='field__label'>Brightness:</label>
      <input className='field__value' type='range' min='0' max='100' step='5' value={monitor.brightness} placeholder='brightness' onInput={(e) => onChange('brightness', parseInt(e.target.value) || 0)} />
    </div>
  </div>
}


// react query store
const QUERY_KEY_MONITORS = 'monitors';
function useMonitors(){
  return useQuery(QUERY_KEY_MONITORS, ApiUtils.getMonitors);
}

function useUpdateMonitor(){
  return useMutation(ApiUtils.updateMonitor);
}


ReactDOM.render(<Home />, document.querySelector('#root'));
