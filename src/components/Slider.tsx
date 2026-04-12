import { useState, useEffect, useRef, useCallback } from "react";

interface SliderProps {
  icon?: string;
  value: number;
  min?: number;
  max?: number;
  onChange: (value: number) => void;
  showValue?: boolean;
  onIconClick?: () => void;
}

export default function Slider({
  icon,
  value,
  min = 0,
  max = 100,
  onChange,
  showValue = true,
  onIconClick,
}: SliderProps) {
  const [localValue, setLocalValue] = useState(value);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setLocalValue(value);
  }, [value]);

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
    [onChange]
  );

  const percentage = ((localValue - min) / (max - min)) * 100;

  return (
    <div className="slider-row">
      {icon && (
        <span
          className={`slider-icon${onIconClick ? " slider-icon-clickable" : ""}`}
          onClick={onIconClick}
        >
          {icon}
        </span>
      )}
      <div className="slider-container">
        <div className="slider-track">
          <div className="slider-fill" style={{ width: `${percentage}%` }} />
        </div>
        <input
          type="range"
          className="slider-input"
          min={min}
          max={max}
          value={localValue}
          onChange={handleChange}
        />
      </div>
      {showValue && <span className="slider-value">{localValue}%</span>}
    </div>
  );
}
