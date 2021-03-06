import Brightness7Icon from '@mui/icons-material/Brightness7';
import IconButton from '@mui/material/IconButton';
import Tooltip from '@mui/material/Tooltip';
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
    try {
      setAllBrightness(brightness);
      await batchUpdateMonitors({ brightness });
    } catch (err) {
      console.error('AllMonitorBrightnessSetting.onChange Failed', err);
    }
    setDisabled(false);
  };

  const onMinAndMaxBrightness = () => {
    if (allBrightness === 0) {
      // maximize brightness
      onChange(100);
    } else {
      // minimize brightness
      onChange(0);
    }
  };

  return (
    <>
      <div className='field'>
        <span className='field__icon'>
          <Tooltip arrow title='Minimize or maximize brightness for all monitors'>
            <IconButton size='small' onClick={onMinAndMaxBrightness}>
              <Brightness7Icon />
            </IconButton>
          </Tooltip>
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
