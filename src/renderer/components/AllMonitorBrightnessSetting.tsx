import Brightness7Icon from '@mui/icons-material/Brightness7';
import { useState } from 'react';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { Slider } from 'src/renderer/components/Slider';
import { useBatchUpdateMonitors } from 'src/renderer/hooks';
import { Monitor } from 'src/types.d';

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
  const [disabled, setDisabled] = useState(false);
  const { mutateAsync: batchUpdateMonitors } = useBatchUpdateMonitors();

  const onChange = async (brightness: number) => {
    setDisabled(true);
    setAllBrightness(brightness);
    await batchUpdateMonitors({ brightness });
    setDisabled(false);
  };

  return (
    <>
      <div className='field'>
        <span className='field__icon' title='All Monitor Brightness'>
          <Brightness7Icon />
        </span>
        <span className='field__slider'>
          <Slider
            className='field__value'
            placeholder='brightness'
            value={allBrightness}
            onInput={onChange}
            disabled={disabled}
          />
        </span>
      </div>
    </>
  );
}
