import Slider from './Slider';

interface AllMonitorsControlProps {
  brightness: number;
  onBrightnessChange: (value: number) => void;
  contrast: number | null;
  onContrastChange: (value: number) => void;
  showContrast: boolean;
  monitorCount: number;
  minBrightness: number;
  onExpand: () => void;
}

/** Combined brightness and optional contrast slider that controls all monitors at once.
 * Includes an expand chevron to switch to individual monitor view. */
export default function AllMonitorsControl({
  brightness,
  onBrightnessChange,
  contrast,
  onContrastChange,
  showContrast,
  monitorCount,
  minBrightness,
  onExpand,
}: AllMonitorsControlProps) {
  return (
    <div className='all-monitors-section'>
      <div className='section-label-row'>
        <span className='section-label'>All Monitors ({monitorCount})</span>
        <button className='section-toggle' onClick={onExpand} title='Show individual monitors'>
          <span className='chevron'>&#9662;</span>
        </button>
      </div>
      <Slider
        icon='☀'
        value={brightness}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(brightness > minBrightness ? minBrightness : 100)}
      />
      {showContrast && contrast !== null && (
        <Slider
          icon={'\u25D0'}
          value={contrast}
          onChange={onContrastChange}
          onIconClick={() => onContrastChange(contrast > 0 ? 0 : 100)}
        />
      )}
    </div>
  );
}
