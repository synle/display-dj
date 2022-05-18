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

  const onBrightnessChange = async (brightness: number) => {
    setDisabled(true);
    await updateMonitor({
      id: monitor.id,
      brightness,
    });
    setDisabled(false);
  };

  return (
    <>
      <div className='field'>
        <MonitorNameInput monitor={monitor} idx={props.idx} />
      </div>
      <div className='field' title='Monitor Brightness'>
        {isLaptop ? (
          <span title='Laptop Display' className='field__icon'>
            <LaptopChromebookIcon />
          </span>
        ) : (
          <span title='Monitor Display' className='field__icon'>
            <MonitorIcon />
          </span>
        )}
        <span className='field__slider'>
          <Slider
            className='field__value'
            placeholder='brightness'
            value={monitor.brightness}
            onInput={onBrightnessChange}
            disabled={disabled}
          />
        </span>
      </div>
    </>
  );
}
