import { Icon } from './Icons';
import RefreshLabel from './RefreshLabel';
import Slider from './Slider';

interface AllMonitorsControlProps {
  brightness: number;
  brightnessMixed?: boolean;
  onBrightnessChange: (value: number) => void;
  contrast: number | null;
  contrastMixed?: boolean;
  onContrastChange: (value: number) => void;
  showContrast: boolean;
  monitorCount: number;
  minBrightness: number;
  onExpand: () => void;
  /** Rescans displays and speakers. */
  onRefresh?: () => void;
}

/** Combined brightness and optional contrast slider that controls all monitors at once.
 * Includes an expand chevron to switch to individual monitor view. */
export default function AllMonitorsControl({
  brightness,
  brightnessMixed = false,
  onBrightnessChange,
  contrast,
  contrastMixed = false,
  onContrastChange,
  showContrast,
  monitorCount,
  minBrightness,
  onExpand,
  onRefresh = () => {},
}: AllMonitorsControlProps) {
  return (
    <div className='all-monitors-section'>
      <div className='section-label-row'>
        <RefreshLabel text={`All Monitors (${monitorCount})`} onRefresh={onRefresh} />
        <button
          className='section-toggle'
          onClick={onExpand}
          title='Show individual monitors'
          aria-expanded='false'
          aria-controls='monitor-controls'>
          <span className='chevron'>
            <Icon name='chevronRight' size={14} />
          </span>
        </button>
      </div>
      <Slider
        label='Brightness for all monitors'
        icon={<Icon name='allDisplays' />}
        iconLabel='Toggle brightness for all monitors'
        value={brightness}
        mixed={brightnessMixed}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(brightness > minBrightness ? minBrightness : 100)}
      />
      {showContrast && contrast !== null && (
        <Slider
          label='Contrast for all monitors'
          icon={'\u25D0'}
          iconLabel='Toggle contrast for all monitors'
          value={contrast}
          mixed={contrastMixed}
          onChange={onContrastChange}
          onIconClick={() => onContrastChange(contrast > 0 ? 0 : 100)}
        />
      )}
    </div>
  );
}
