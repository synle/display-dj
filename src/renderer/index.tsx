import ReactDOM from 'react-dom';
import {
  QueryClient,
  QueryClientProvider,
  useMutation,
  useQuery,
  useQueryClient,
} from 'react-query';
import React, { useEffect, useState } from 'react';
import ApiUtils from 'src/renderer/utils/ApiUtils';
import { LAPTOP_BUILT_IN_DISPLAY_ID, DISPLAY_TYPE } from 'src/constants';
import MonitorSvg from 'src/renderer/svg/monitor.svg';
import LaptopSvg from 'src/renderer/svg/laptop.svg';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import './index.scss';
import { Monitor, MonitorUpdateInput } from 'src/types.d';
// TODO: extract these things
const queryClient = new QueryClient();

// react components
type HomeProps = {};
function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs } = useConfigs();
  const { isLoading: loadingAppState, data: appState } = useAppState();
  const { mutateAsync: updateAppHeight } = useUpdateAppHeight();

  useEffect(() => {
    const config = { childList: true, subtree: true };
    const callback = () => updateAppHeight();

    const observer = new MutationObserver(callback);
    observer.observe(document.body, config);

    return () => {
      observer.disconnect();
    };
  }, []);
  const isLoading = loadingConfigs || loadingAppState;
  if (isLoading) {
    return <div style={{ padding: '2rem 1rem', fontSize: '1.25rem' }}>Loading...</div>;
  }

  if (!configs || !appState) {
    // TODO: add message for no data state
    return (
      <div style={{ padding: '2rem 1rem', fontSize: '1.25rem' }}>Errors. Get configs failed...</div>
    );
  }

  return (
    <>
      <header>
        <h2>
          Display-DJ {configs.version} {configs.env !== 'production' ? configs.env : ''}
        </h2>
        <ToggleAllDisplay />
      </header>
      {appState.expanded ? (
        <MonitorBrightnessSettingForm monitors={configs.monitors} />
      ) : (
        <AllMonitorBrightnessSettings monitors={configs.monitors} />
      )}
      <DarkModeSettingForm darkMode={configs.darkMode} />
    </>
  );
}

function ToggleAllDisplay() {
  const { isLoading, data: appState } = useAppState();
  const { mutateAsync: updateAppState } = useUpdateAppState();

  if (isLoading || !appState) {
    return null;
  }

  const onToggleAll = () => {
    appState.expanded = !appState.expanded;
    updateAppState(appState);
  };

  return (
    <span
      className='iconBtn'
      onClick={onToggleAll}
      title='Toggle brightness for individual display'>
      <ToggleSvg />
    </span>
  );
}

type DarkModeSettingFormProps = {
  darkMode: boolean;
};
function DarkModeSettingForm(props: DarkModeSettingFormProps) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState<boolean>(darkModeFromProps);
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

type MonitorBrightnessSettingFormProps = {
  monitors: Monitor[];
};
function MonitorBrightnessSettingForm(props: MonitorBrightnessSettingFormProps) {
  const { monitors } = props;
  return (
    <>
      {monitors
        .filter((monitor) => !monitor.disabled)
        .map((monitor, idx) => (
          <MonitorBrightnessSetting key={monitor.id} monitor={monitor} idx={idx + 1} />
        ))}
    </>
  );
}

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};
function MonitorBrightnessSetting(props: MonitorBrightnessSettingProps) {
  const [editName, setEditName] = useState(false);
  const [name, setName] = useState('');
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const queryClient = useQueryClient();
  const isLaptop = monitor.type === DISPLAY_TYPE.LAPTOP;

  const onChange = (key: string, value: any) => {
    //@ts-ignore
    monitor[key] = value;
    updateMonitor(monitor);
  };

  const onNameBlur = (e: React.SyntheticEvent) => {
    e.preventDefault();

    monitor.name = name.trim();

    if (!monitor.name) {
      monitor.name = `Monitor #${props.idx}`;
    }

    setEditName(false);
    updateMonitor(monitor);
  };

  useEffect(() => {
    setName(monitor.name);
  }, [monitor.name]);

  return (
    <>
      <div className='field'>
        {editName ? (
          <form onSubmit={onNameBlur}>
            <input
              className='field__value'
              value={name}
              placeholder='Enter a display name'
              autoFocus={true}
              onInput={(e) => setName((e.target as HTMLInputElement).value)}
              onBlur={onNameBlur}
              required
              type='text'
            />
          </form>
        ) : (
          <div className='field__value field__value-readonly'>
            <a onClick={() => setEditName(true)} title='Monitor Name' href='#'>
              {monitor.name}
            </a>
          </div>
        )}
      </div>
      <div className='field' title='Monitor Brightness'>
        {isLaptop ? (
          <span title='Laptop Display' className='field__icon'>
            <LaptopSvg />
          </span>
        ) : (
          <span title='Monitor Display' className='field__icon'>
            <MonitorSvg />
          </span>
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

type AllMonitorBrightnessSettingsProps = {
  monitors: Monitor[];
};
function AllMonitorBrightnessSettings(props: AllMonitorBrightnessSettingsProps) {
  const { monitors } = props;
  const allBrightnessValueFromProps = Math.min(
    ...monitors.map((monitor) => monitor.brightness),
    100,
  );
  const [allBrightness, setAllBrightness] = useState(allBrightnessValueFromProps);
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const { mutateAsync: updateAppState } = useUpdateAppState();
  const queryClient = useQueryClient();

  const onChange = async (value: number) => {
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
          All Monitors ({monitors.length})
        </div>
      </div>
      <div className='field'>
        <span className='field__icon' title='All Monitor Brightness'>
          🔆
        </span>
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

const QUERY_KEY_APP_STATE = 'appState';

type AppState = {
  expanded: boolean;
};
let _appState: AppState = { expanded: false };
function useAppState() {
  return useQuery(QUERY_KEY_APP_STATE, () => _appState);
}

function useConfigs() {
  return useQuery(QUERY_KEY_CONFIGS, ApiUtils.getConfigs);
}

function useUpdateMonitor() {
  return useMutation(ApiUtils.updateMonitor);
}

function useToggleDarkMode() {
  return useMutation(ApiUtils.updateDarkMode);
}

function useUpdateAppHeight() {
  return useMutation(ApiUtils.updateAppHeight);
}

function useUpdateAppState() {
  return useMutation<void, void, AppState>(
    async (newAppState: AppState) => {
      _appState = { ..._appState, ...newAppState };
    },
    {
      onSuccess: () => {
        queryClient.invalidateQueries(QUERY_KEY_CONFIGS);
      },
    },
  );
}

// render the main app
ReactDOM.render(
  <QueryClientProvider client={queryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
