import Slider from './Slider';

interface AllMonitorsControlProps {
  brightness: number;
  onBrightnessChange: (value: number) => void;
  contrast: number | null;
  onContrastChange: (value: number) => void;
  showContrast: boolean;
  monitorCount: number;
  minBrightness: number;
}

/** Combined brightness and optional contrast slider that controls all monitors at once. */
export default function AllMonitorsControl({
  brightness,
  onBrightnessChange,
  contrast,
  onContrastChange,
  showContrast,
  monitorCount,
  minBrightness,
}: AllMonitorsControlProps) {
  return (
    <div className='all-monitors-section'>
      <div className='section-label'>All Monitors ({monitorCount})</div>
      <Slider
        icon='☀'
        value={brightness}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(brightness > minBrightness ? minBrightness : 100)}
      />
      {showContrast && contrast !== null && (
        <Slider
          icon={'\uD83D\uDD06'}
          value={contrast}
          onChange={onContrastChange}
          onIconClick={() => onContrastChange(contrast > 0 ? 0 : 100)}
        />
      )}
    </div>
  );
}
