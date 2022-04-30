import React, { useMemo, useState, useEffect } from 'react';
import { debounce } from 'src/renderer/utils/CommonUtils';

type SliderProps = {
  value?: number;
  onInput: (e: React.FormEvent<HTMLInputElement>) => void;
  className?: string;
  placeholder?: string;
  disabled?: boolean;
};

const DEBOUNCE_TIME_MS = 200;

export function Slider(props: SliderProps) {
  const [tempVal, setTempVal] = useState(0);

  const { value, onInput, className, placeholder, disabled } = props;

  const debouncedOnInput = useMemo(() => debounce(onInput, DEBOUNCE_TIME_MS), []);

  const onChange = (e: React.FormEvent<HTMLInputElement>) => {
    setTempVal(parseInt((e.target as HTMLInputElement).value) || 0);
    debouncedOnInput(e);
  }

  useEffect(() => {
    let newValue = value;
    if(newValue === undefined || newValue <= 0){
      newValue = 0;
    }

    if(newValue > 100){
      newValue = 100;
    }
    setTempVal(newValue);
  }, [value])

  return (
    <input
      type='range'
      min='0'
      max='100'
      step='10'
      value={tempVal}
      onInput={onChange}
      className={className}
      placeholder={placeholder}
      disabled={disabled}
      autoFocus={true}
    />
  );
}
