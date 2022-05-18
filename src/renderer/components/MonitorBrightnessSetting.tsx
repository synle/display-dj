import LaptopChromebookIcon from '@mui/icons-material/LaptopChromebook';
import MonitorIcon from '@mui/icons-material/Monitor';
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
    await updateMonitor({
      id: monitor.id,
      brightness,
    });
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
        <span
          title='Minimize or maximize brightness for this monitor'
          className='field__icon field__button'
          onClick={onMinAndMaxBrightness}>
          {isLaptop ? <LaptopChromebookIcon /> : <MonitorIcon />}
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
