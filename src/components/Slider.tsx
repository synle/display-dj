import { useState, useEffect, useRef, useCallback } from 'react';

interface SliderProps {
  icon?: string;
  value: number;
  min?: number;
  max?: number;
  /**
   * Step size for the underlying range input. Defaults to 1 (continuous).
   * Used by the keyboard-backlight slider to snap to 25% increments so the
   * only reachable values are 0/25/50/75/100 — matches the same snapping
   * the backend applies to shortcut commands.
   */
  step?: number;
  onChange: (value: number) => void;
  showValue?: boolean;
  unit?: string;
  onIconClick?: () => void;
}

/** Reusable range slider with optional icon, debounced onChange, and value display. */
export default function Slider({
  icon,
  value,
  min = 0,
  max = 100,
  step = 1,
  onChange,
  showValue = true,
  unit = '%',
  onIconClick,
}: SliderProps) {
  const [localValue, setLocalValue] = useState(value);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalValue(value);
  }, [value]);

  /** Debounces slider input to avoid flooding the backend with brightness/volume calls. */
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newValue = Number(e.target.value);
      setLocalValue(newValue);

      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
      timeoutRef.current = setTimeout(() => {
        onChange(newValue);
      }, 150);
    },
    [onChange],
  );

  const percentage = ((localValue - min) / (max - min)) * 100;

  return (
    <div className='slider-row'>
      {icon && (
        <span
          className={`slider-icon${onIconClick ? ' slider-icon-clickable' : ''}`}
          onClick={onIconClick}>
          {icon}
        </span>
      )}
      <div className='slider-container'>
        <div className='slider-track'>
          <div className='slider-fill' style={{ width: `${percentage}%` }} />
        </div>
        <input
          type='range'
          className='slider-input'
          min={min}
          max={max}
          step={step}
          value={localValue}
          onChange={handleChange}
        />
      </div>
      {showValue && (
        <span className='slider-value'>
          {localValue}
          {unit}
        </span>
      )}
    </div>
  );
}
