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

// TODO: extract these things
const queryClient = new QueryClient();

// react components
function Home(props) {
  const { isLoading: loadingMonitors, data: monitors } = useMonitors();
  const { isLoading: loadingConfigs, data: configs } = useConfigs();

  const isLoading = loadingMonitors || loadingConfigs;

  if (isLoading) {
    return <>Loading...</>;
  }

  if (!monitors || monitors.length === 0) {
    // TODO: add message for no data state
    return null;
  }

  return (
    <>
      <header>
        <h1>Display-DJ</h1>
      </header>
      <MonitorBrightnessSettingForm monitors={monitors} />
      <AllMonitorBrightnessSettings monitors={monitors} />
      <DarkModeSettingForm configs={configs} />
    </>
  );
}

function DarkModeSettingForm(props) {
  const darkModeFromProps = props.configs.darkMode === true;
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
  const [allBrightness, setAllBrightness] = useState(50);
  const { monitors } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const queryClient = useQueryClient();

  const onChange = async (value) => {
    setAllBrightness(value);

    for (const monitor of monitors) {
      monitor.brightness = value;
      await updateMonitor(monitor);
    }

    queryClient.invalidateQueries(QUERY_KEY_MONITORS);
  };

  return (
    <>
      <div className='field'>
        <div className='field__value field__value-readonly' title='All Monitors'>
          All Monitors
        </div>
      </div>
      <div className='field'>
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
const QUERY_KEY_MONITORS = 'monitors';

const QUERY_KEY_CONFIGS = 'configs';

function useMonitors() {
  return useQuery(QUERY_KEY_MONITORS, ApiUtils.getMonitors);
}

function useUpdateMonitor() {
  return useMutation(ApiUtils.updateMonitor);
}

function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

function useToggleDarkMode() {
  return useMutation(ApiUtils.toggleDarkMode);
}
ReactDOM.render(
  <QueryClientProvider client={queryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
