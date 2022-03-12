import ReactDOM from 'react-dom';
import React, {useState, useEffect} from 'react';
import { QueryClient, QueryClientProvider, useQuery, useMutation, useQueryClient } from 'react-query';
import ApiUtils from './utils/ApiUtils';

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
    <DarkModeSettingForm />
    <MonitorBrightnessSettingForm />
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

  return data.map(monitor => <MonitorBrightnessSetting key={monitor.id} monitor={monitor}/> );
}

function MonitorBrightnessSetting(props){
  const {monitor } = props;
  const {mutateAsync: updateMonitor} = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = async (key, value) => {
    monitor[key] = value;
    await updateMonitor(monitor);
  }

  return <div>
    <div className='frow'>
      <div className='flabel'>ID:</div>
      <div className='fvalue'>{monitor.id}</div>
    </div>
    <div className='frow'>
      <div className='flabel'>Name:</div>
      <input className='fvalue' value={monitor.name} placeholder='name' onInput={(e) => onChange('name', e.target.value)} />
    </div>
    <div className='frow'>
      <div className='flabel'>Brightness:</div>
      <input className='fvalue' type='range' min='0' max='100' step='5' value={monitor.brightness} placeholder='brightness' onInput={(e) => onChange('brightness', parseInt(e.target.value) || 0)} />
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
