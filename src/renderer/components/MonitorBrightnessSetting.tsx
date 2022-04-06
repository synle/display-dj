import { MonitorNameInput } from 'src/renderer/components/MonitorNameInput';
import { Slider } from 'src/renderer/components/Slider';
import { LAPTOP_BUILT_IN_DISPLAY_ID } from 'src/constants';
import MonitorSvg from 'src/renderer/svg/monitor.svg';
import LaptopSvg from 'src/renderer/svg/laptop.svg';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import DarkModeSvg from 'src/renderer/svg/darkMode.svg';
import LightModeSvg from 'src/renderer/svg/lightMode.svg';
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
  QUERY_KEY_CONFIGS,
  QUERY_KEY_APP_STATE,
} from 'src/renderer/hooks';

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};

export function MonitorBrightnessSetting(props: MonitorBrightnessSettingProps) {
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();

  const isLaptop = monitor.type === 'laptop_monitor';

  const onBrightnessChange = async (brightness: number) => {
    await updateMonitor({
      id: monitor.id,
      brightness,
    });
  };

  return (
    <>
      <div className='field'>
        <MonitorNameInput monitor={monitor} idx={props.idx} />
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
