import LaptopChromebookIcon from '@mui/icons-material/LaptopChromebook';
import MonitorIcon from '@mui/icons-material/Monitor';
import IconButton from '@mui/material/IconButton';
import Tooltip from '@mui/material/Tooltip';
import { useState } from 'react';
import { MonitorNameInput } from 'src/renderer/components/MonitorNameInput';
import { Slider } from 'src/renderer/components/Slider';
import { useUpdateMonitor } from 'src/renderer/hooks';
import { Monitor } from 'src/types.d';

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};

export function MonitorBrightnessSetting(props: MonitorBrightnessSettingProps) {
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();
  const [disabled, setDisabled] = useState(false);
  const isLaptop = monitor.type === 'laptop_monitor';

  const onChange = async (brightness: number) => {
    setDisabled(true);
    try{
      await updateMonitor({
      id: monitor.id,
      brightness,
    });
    } catch(err){
      console.error('MonitorBrightnessSetting.onChange failed', err);
    }
    setDisabled(false);
  };

  const onMinAndMaxBrightness = () => {
    if (monitor.brightness === 0) {
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
        <MonitorNameInput monitor={monitor} idx={props.idx} />
      </div>
      <div className='field' title='Monitor Brightness'>
        <span className='field__icon'>
          <Tooltip arrow title='Minimize or maximize brightness for this monitor'>
            <IconButton size='small' onClick={onMinAndMaxBrightness}>
              {isLaptop ? <LaptopChromebookIcon /> : <MonitorIcon />}
            </IconButton>
          </Tooltip>
        </span>
        <span className='field__slider'>
          <Slider
            className='field__value'
            placeholder='brightness'
            value={monitor.brightness}
            onInput={onChange}
            disabled={disabled}
          />
        </span>
      </div>
    </>
  );
}
