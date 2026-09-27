import { useState, useRef, useCallback } from 'react';

interface SliderProps {
  label: string;
  icon?: string;
  iconLabel?: string;
  value: number;
  min?: number;
  max?: number;
  onChange: (value: number) => void;
  showValue?: boolean;
  unit?: string;
  onIconClick?: () => void;
  mixed?: boolean;
}

/** Reusable range slider with optional icon, debounced onChange, and value display. */
export default function Slider({
  label,
  icon,
  iconLabel,
  value,
  min = 0,
  max = 100,
  onChange,
  showValue = true,
  unit = '%',
  onIconClick,
  mixed = false,
}: SliderProps) {
  const [localValue, setLocalValue] = useState(value);
  const [prevPropValue, setPrevPropValue] = useState(value);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Sync external value changes (backend updates, profile applies) into local
  // state by adjusting during render — the pattern React recommends instead of
  // a setState-in-effect round trip. Drag edits stay local; only prop changes
  // reset.
  if (value !== prevPropValue) {
    setPrevPropValue(value);
    setLocalValue(value);
  }

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
      }, 50);
    },
    [onChange],
  );

  const percentage = ((localValue - min) / (max - min)) * 100;

  return (
    <div className='slider-row'>
      {icon &&
        (onIconClick ? (
          <button
            type='button'
            className='slider-icon slider-icon-clickable'
            aria-label={iconLabel ?? `Toggle ${label.toLowerCase()}`}
            onClick={onIconClick}>
            {icon}
          </button>
        ) : (
          <span className='slider-icon' aria-hidden='true'>
            {icon}
          </span>
        ))}
      <div className='slider-container'>
        <div className='slider-track'>
          <div className='slider-fill' style={{ width: `${percentage}%` }} />
        </div>
        <input
          type='range'
          className='slider-input'
          min={min}
          max={max}
          value={localValue}
          aria-label={label}
          aria-valuetext={mixed ? `Mixed, ${localValue}${unit}` : undefined}
          onChange={handleChange}
        />
      </div>
      {showValue && (
        <span className={`slider-value${mixed ? ' slider-value-mixed' : ''}`}>
          {mixed ? 'Mixed' : `${localValue}${unit}`}
        </span>
      )}
    </div>
  );
}
