import ReactDOM from 'react-dom';
import { QueryClient, QueryClientProvider } from 'react-query';
import React, { useEffect, useMemo, useState } from 'react';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import MonitorSvg from 'src/renderer/svg/monitor.svg';
import LaptopSvg from 'src/renderer/svg/laptop.svg';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import DarkModeSvg from 'src/renderer/svg/darkMode.svg';
import LightModeSvg from 'src/renderer/svg/lightMode.svg';
import './index.scss';
import { Monitor, SingleMonitorUpdateInput, Preference, AppConfig, Volume } from 'src/types.d';
import {
  useBatchUpdateMonitors,
  usePreferences,
  useUpdatePreferences,
  useConfigs,
  useUpdateMonitor,
  useToggleDarkMode,
  useUpdateAppPosition,
  useUpdateVolume,
  useUpdateMuted,
  QUERY_KEY_CONFIGS,
  QUERY_KEY_APP_STATE,
} from 'src/renderer/hooks';
import { debounce } from 'src/renderer/utils/CommonUtils';

// react components
type HomeProps = {};
function Home(props: HomeProps) {
  const { isLoading: loadingConfigs, data: configs, refetch } = useConfigs();
  const { isLoading: loadingPrefs, data: preference } = usePreferences();
  const { mutateAsync: updateAppPosition } = useUpdateAppPosition();

  useEffect(() => {
    const config = { childList: true, subtree: true };
    const callback = () => updateAppPosition();

    const observer = new MutationObserver(callback);
    observer.observe(document.body, config);

    // update position and refetched and the page is visible
    const onVisibilityChange = () => {
      updateAppPosition();
      refetch();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      observer.disconnect();
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, []);

  const isLoading = loadingConfigs || loadingPrefs;
  if (isLoading) {
    return (
      <>
        <Header configs={configs} preference={preference} />
        <h3>Loading... Please wait...</h3>
      </>
    );
  }

  if (!configs || !preference) {
    // TODO: add message for no data state
    return (
      <>
        <Header configs={configs} preference={preference} />
        <h3>Errors... Failed to get data...</h3>
      </>
    );
  }

  return (
    <>
      <Header configs={configs} preference={preference} />
      {preference.showIndividualDisplays ? (
        <MonitorBrightnessSettingForm monitors={configs.monitors} />
      ) : (
        <AllMonitorBrightnessSetting monitors={configs.monitors} />
      )}
      {configs.volume.isDisabled === false && <VolumeSetting volume={configs.volume} />}
      <DarkModeSettingForm darkMode={configs.darkMode} />
    </>
  );
}

type HeaderProps = {
  configs?: AppConfig;
  preference?: Preference;
};
function Header(props: HeaderProps) {
  const { configs, preference } = props;

  return (
    <header>
      <h2>
        Display-DJ {configs?.version} {configs?.env !== 'production' ? configs?.env : ''}
      </h2>
      {!preference ? null : <ToggleAllDisplay preference={preference} />}
    </header>
  );
}

type ToggleAllDisplayProps = {
  preference: Preference;
};
function ToggleAllDisplay(props: ToggleAllDisplayProps) {
  const { preference } = props;
  const { mutateAsync: updatePreferences } = useUpdatePreferences();

  const onToggleAll = () => {
    preference.showIndividualDisplays = !preference.showIndividualDisplays;
    updatePreferences(preference);
  };

  return (
    <span
      className='iconBtn'
      onClick={onToggleAll}
      title={
        preference.showIndividualDisplays
          ? 'Collapse individual displays brightness'
          : 'Expand individual displays brightness'
      }>
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
  const { mutateAsync: updateDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async () => {
    const newDarkMode = !darkMode;

    setDarkMode(newDarkMode);

    await updateDarkMode(newDarkMode);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  return (
    <button className='btnToggleDarkMode' onClick={onToggleDarkMode} title='Toggle Dark Mode'>
      {darkMode ? (
        <>
          <DarkModeSvg /> <span>Mode: Dark</span>
        </>
      ) : (
        <>
          <LightModeSvg />
          <span>Mode: Light</span>
        </>
      )}
    </button>
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

  const isLaptop = monitor.type === 'laptop_monitor';

  const onBrightnessChange = (brightness: number) => {
    updateMonitor({
      id: monitor.id,
      brightness,
    });
  };

  const onDisplayNameChange = (e: React.SyntheticEvent) => {
    e.preventDefault();
    setEditName(false);

    let nameToUse = (name || '').trim() || `Monitor #${props.idx}`;

    updateMonitor({
      id: monitor.id,
      name: nameToUse,
    });
  };

  useEffect(() => {
    setName(monitor.name);
  }, [monitor.name]);

  return (
    <>
      <div className='field'>
        {editName ? (
          <form onSubmit={onDisplayNameChange}>
            <input
              className='field__value'
              value={name}
              placeholder='Enter a display name'
              autoFocus={true}
              onInput={(e) => setName((e.target as HTMLInputElement).value)}
              onBlur={onDisplayNameChange}
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
        <Slider
          className='field__value'
          placeholder='brightness'
          value={monitor.brightness}
          onInput={(e) => onBrightnessChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}

type AllMonitorBrightnessSettingProps = {
  monitors: Monitor[];
};
function AllMonitorBrightnessSetting(props: AllMonitorBrightnessSettingProps) {
  const { monitors } = props;
  const allBrightnessValueFromProps = Math.min(
    ...monitors.map((monitor) => monitor.brightness),
    100,
  );
  const [allBrightness, setAllBrightness] = useState(allBrightnessValueFromProps);
  const { mutateAsync: batchUpdateMonitors } = useBatchUpdateMonitors();

  const onChange = async (brightness: number) => {
    setAllBrightness(brightness);
    await batchUpdateMonitors({ brightness });
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
        <Slider
          className='field__value'
          placeholder='brightness'
          value={allBrightness}
          onInput={(e) => onChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}

type VolumeSettingProps = {
  volume: Volume;
};
function VolumeSetting(props: VolumeSettingProps) {
  const { volume } = props;

  const { mutateAsync: updateVolume } = useUpdateVolume();
  const { mutateAsync: updateMuted } = useUpdateMuted();

  const onChange = (newValue: number) => updateVolume(newValue);
  const onSetMuted = () => updateMuted(!volume.muted);

  return (
    <>
      <div className='field'>
        <div className='field__value field__value-readonly' title='Volume'>
          Volume
        </div>
      </div>
      <div className='field'>
        <span className='field__icon iconBtn' title='Toggle Muted' onClick={onSetMuted}>
          <VolumeIcon volume={volume} />
        </span>
        <Slider
          key={volume.value}
          className='field__value'
          placeholder='Volume'
          value={volume.value}
          onInput={(e) => onChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}

function VolumeIcon(props: VolumeSettingProps) {
  const { volume } = props;

  const icon = useMemo(() => {
    if (volume.muted) {
      return '🔇';
    }

    const { value } = volume;
    if (value < 30) {
      return '🔈';
    }
    if (value < 60) {
      return '🔉';
    }
    return '🔊';
  }, [volume.muted, volume.value]);
  return <>{icon}</>;
}

type SliderProps = {
  value?: number;
  onInput: (e: React.FormEvent<HTMLInputElement>) => void;
  className?: string;
  placeholder?: string;
  disabled?: boolean;
};
function Slider(props: SliderProps) {
  const { value, onInput, className, placeholder, disabled } = props;

  const debouncedOnInput = useMemo(() => debounce(onInput, 200), []);

  return (
    <input
      type='range'
      min='0'
      max='100'
      step='10'
      defaultValue={value}
      onInput={debouncedOnInput}
      className={className}
      placeholder={placeholder}
      disabled={disabled}
      autoFocus={true}
    />
  );
}

// render the main app
const appQueryClient = new QueryClient();
ReactDOM.render(
  <QueryClientProvider client={appQueryClient}>
    <Home />
  </QueryClientProvider>,
  document.querySelector('#root'),
);
