import { useState, useRef } from 'react';
import { Monitor } from '../types';
import { Icon } from './Icons';
import Slider from './Slider';

interface MonitorControlProps {
  monitor: Monitor;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
  showContrast: boolean;
  onRename: (name: string) => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
  isFirst?: boolean;
  isLast?: boolean;
  minBrightness: number;
}

/** Individual monitor control: editable name label, brightness slider, optional contrast slider, and reorder buttons. */
export default function MonitorControl({
  monitor,
  onBrightnessChange,
  onContrastChange,
  showContrast,
  onRename,
  onMoveUp,
  onMoveDown,
  isFirst,
  isLast,
  minBrightness,
}: MonitorControlProps) {
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(monitor.name);
  const inputRef = useRef<HTMLInputElement>(null);

  /** Enters inline rename mode and focuses the input. */
  const startEditing = () => {
    setEditName(monitor.name);
    setEditing(true);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  /** Commits the rename (or reverts on empty/unchanged input). */
  const finishEditing = () => {
    setEditing(false);
    const trimmed = editName.trim();
    if (trimmed === monitor.name) return;
    // Empty input clears the custom name, reverting to the API default
    onRename(trimmed);
  };

  /** Handles Enter (commit) and Escape (cancel) during inline rename. */
  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      finishEditing();
    } else if (e.key === 'Escape') {
      setEditing(false);
      setEditName(monitor.name);
    }
  };

  return (
    <div className='monitor-control'>
      <div className='monitor-name-row'>
        {editing ? (
          <input
            ref={inputRef}
            className='monitor-name-input'
            value={editName}
            placeholder={monitor.originalName}
            onChange={(e) => setEditName(e.target.value)}
            onBlur={finishEditing}
            onKeyDown={handleKeyDown}
          />
        ) : (
          <button className='monitor-name' onClick={startEditing}>
            {monitor.name || monitor.originalName}
          </button>
        )}
        {onMoveUp && onMoveDown && (
          <div className='monitor-reorder-buttons'>
            <button
              className='monitor-reorder-btn'
              onClick={onMoveUp}
              disabled={isFirst}
              title='Move up'>
              <Icon name='chevronUp' size={12} />
            </button>
            <button
              className='monitor-reorder-btn'
              onClick={onMoveDown}
              disabled={isLast}
              title='Move down'>
              <Icon name='chevronDown' size={12} />
            </button>
          </div>
        )}
      </div>
      <Slider
        label={`Brightness for ${monitor.name || monitor.originalName}`}
        icon={<Icon name={monitor.isBuiltIn ? 'laptop' : 'monitor'} />}
        iconLabel={`Toggle brightness for ${monitor.name || monitor.originalName}`}
        value={monitor.brightness}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() =>
          onBrightnessChange(monitor.brightness > minBrightness ? minBrightness : 100)
        }
      />
      {showContrast && monitor.contrast !== null && (
        <Slider
          label={`Contrast for ${monitor.name || monitor.originalName}`}
          icon={'\u25D0'}
          iconLabel={`Toggle contrast for ${monitor.name || monitor.originalName}`}
          value={monitor.contrast}
          onChange={onContrastChange}
          onIconClick={() => onContrastChange(monitor.contrast! > 0 ? 0 : 100)}
        />
      )}
    </div>
  );
}
