import React, { useMemo } from 'react';
import { debounce } from 'src/renderer/utils/CommonUtils';

type SliderProps = {
  value?: number;
  onInput: (e: React.FormEvent<HTMLInputElement>) => void;
  className?: string;
  placeholder?: string;
  disabled?: boolean;
};

export function Slider(props: SliderProps) {
  const { value, onInput, className, placeholder, disabled } = props;

  const debouncedOnInput = useMemo(() => debounce(onInput, 400), []);

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
