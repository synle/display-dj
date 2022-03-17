import ReactDOM from 'react-dom';
import {
  QueryClient,
  QueryClientProvider,
  useMutation,
  useQuery,
  useQueryClient,
} from 'react-query';
import { useEffect, useState } from 'react';
import ApiUtils from './utils/ApiUtils';
import './index.scss';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from '../constants';

// TODO: extract these things
const queryClient = new QueryClient();

// react components
function Home(props) {
  const { isLoading, data: configs } = useConfigs();

  if (isLoading) {
    return <>Loading...</>;
  }

  if (!configs) {
    // TODO: add message for no data state
    return <>Errors. Get configs failed...</>;
  }

  return (
    <>
      <header>
        <h2>Display-DJ 1.0.0</h2>
      </header>
      <MonitorBrightnessSettingForm monitors={configs.monitors} />
      <AllMonitorBrightnessSettings monitors={configs.monitors} />
      <DarkModeSettingForm darkmode={configs.darkmode} />
    </>
  );
}

function DarkModeSettingForm(props) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState(darkModeFromProps);
  const { mutateAsync: toggleDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async () => {
    const newDarkMode = !darkMode;

    setDarkMode(newDarkMode);

    await toggleDarkMode(newDarkMode);

    queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  return (
    <div className='field field__darkmode'>
      <button onClick={onToggleDarkMode} title='Toggle Dark Mode'>
        {darkMode ? 'Mode: Light' : 'Mode: Dark'}
      </button>
    </div>
  );
}

function MonitorBrightnessSettingForm(props) {
  const { monitors } = props;
  return monitors.map((monitor, idx) => (
    <MonitorBrightnessSetting key={monitor.id} monitor={monitor} idx={idx + 1} />
  ));
}

function MonitorBrightnessSetting(props) {
  const [editName, setEditName] = useState(false);
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = (key, value) => {
    monitor[key] = value;
    updateMonitor(monitor);
  };

  const onNameBlur = () => {
    monitor.name = monitor.name.trim();

    if (!monitor.name) {
      monitor.name = `Monitor #${props.idx}`;
    }

    setEditName(false);
    updateMonitor(monitor);
  };

  const isLaptop = monitor.id === LAPTOP_BUILT_IN_DISPLAY_ID;

  return (
    <>
      <div className='field'>
        {editName ? (
          <input
            className='field__value'
            value={monitor.name}
            placeholder='name'
            autoFocus={true}
            onInput={(e) => onChange('name', e.target.value)}
            onBlur={onNameBlur}
          />
        ) : (
          <div className='field__value field__value-readonly'>
            <a onClick={() => setEditName(true)} title='Monitor Name'>
              {monitor.name}
            </a>
          </div>
        )}
      </div>
      <div className='field' title='Monitor Brightness'>
        {isLaptop ? (
          <span title='Laptop Display'>💻</span>
        ) : (
          <span title='Monitor Display'>🖥️</span>
        )}
        <input
          className='field__value'
          type='range'
          min='0'
          max='100'
          step='5'
          value={monitor.brightness}
          placeholder='brightness'
          onChange={(e) => onChange('brightness', parseInt(e.target.value) || 0)}
        />
      </div>
    </>
  );
}

function AllMonitorBrightnessSettings(props) {
  const { monitors } = props;
  const allBrightnessValueFromProps = Math.min(
    ...monitors.map((monitor) => monitor.brightness),
    100,
  );
  const [allBrightness, setAllBrightness] = useState(allBrightnessValueFromProps);
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = async (value) => {
    setAllBrightness(value);

    for (const monitor of monitors) {
      monitor.brightness = value;
      await updateMonitor(monitor);
    }

    queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
  };

  return (
    <>
      <div className='field'>
        <div className='field__value field__value-readonly' title='All Monitors'>
          All Monitors
        </div>
      </div>
      <div className='field'>
        <span title='All Monitor Brightness'>🔆</span>
        <input
          className='field__value'
          type='range'
          min='0'
          max='100'
          step='5'
          value={allBrightness}
          placeholder='brightness'
          onChange={(e) => onChange(parseInt(e.target.value) || 0)}
        />
      </div>
    </>
  );
}
// react query store
const QUERY_KEY_CONFIGS = 'configs';

function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

function useUpdateMonitor() {
  return useMutation(ApiUtils.updateMonitor);
}

function useToggleDarkMode() {
  return useMutation(ApiUtils.toggleDarkMode);
}

// render the main app
ReactDOM.render(
  <QueryClientProvider client={queryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
