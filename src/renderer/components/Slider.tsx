import MuiSlider from '@mui/material/Slider';
import { useEffect, useMemo, useState } from 'react';
import { debounce } from 'src/renderer/utils/CommonUtils';

type SliderProps = {
  value?: number;
  onInput: (newVal: number) => void;
  className?: string;
  placeholder?: string;
  disabled?: boolean;
};

const DEBOUNCE_TIME_MS = 800;

const SLIDER_STEP = 10;

export function Slider(props: SliderProps) {
  const [tempVal, setTempVal] = useState(0);

  const { value, onInput, className, placeholder, disabled } = props;

  const debouncedOnInput = useMemo(() => debounce(onInput, DEBOUNCE_TIME_MS), []);

  const onChange = (newVal: number) => {
    setTempVal(newVal);
    debouncedOnInput(newVal);
  };

  useEffect(() => {
    let newValue = value;
    if (newValue === undefined || newValue <= 0) {
      newValue = 0;
    }

    if (newValue > 100) {
      newValue = 100;
    }
    setTempVal(newValue);
  }, [value]);

  return (
    <MuiSlider
      size='small'
      aria-label='Default'
      valueLabelDisplay='auto'
      value={tempVal}
      onChange={(_e, val, _thumb) => onChange(val as number)}
      min={0}
      max={100}
      step={SLIDER_STEP}
      disabled={disabled}
    />
  );
}
