import { useState } from 'react';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { Slider } from 'src/renderer/components/Slider';
import { useBatchUpdateMonitors } from 'src/renderer/hooks';
import { Monitor } from 'src/types.d';
import Brightness7Icon from '@mui/icons-material/Brightness7';

type AllMonitorBrightnessSettingProps = {
  monitors: Monitor[];
};

export function AllMonitorBrightnessSetting(props: AllMonitorBrightnessSettingProps) {
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
        <span className='field__icon' title='All Monitor Brightness'>
          <Brightness7Icon />
        </span>
        <Slider
          className='field__value'
          placeholder='brightness'
          value={allBrightness}
          key={allBrightness}
          onInput={(e) => onChange(parseInt((e.target as HTMLInputElement).value) || 0)}
        />
      </div>
    </>
  );
}
