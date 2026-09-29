import RefreshLabel from './RefreshLabel';
import { useRef, useState } from 'react';
import { AudioOutputDevice, AudioOutputState } from '../types';
import { Icon, type IconName } from './Icons';

/** Volume at or below this shows the medium (one-wave) speaker icon. */
const MEDIUM_VOLUME_MAX = 50;

/** Picks the speaker icon for a volume: muted at 0, medium up to 50%, full above. */
function speakerIconFor(value: number): IconName {
  if (value === 0) return 'speakerMuted';
  return value <= MEDIUM_VOLUME_MAX ? 'speakerMedium' : 'speaker';
}
import Slider from './Slider';

interface VolumeControlProps {
  value: number;
  onChange: (value: number) => void;
  /** Rescans displays and speakers. */
  onRefresh?: () => void;
  outputState: AudioOutputState | null;
  expanded: boolean;
  onToggleExpanded: () => void;
  updatingDeviceId: string | null;
  onSelectOutput: (id: string) => void;
  onRenameOutput: (id: string, label: string) => void;
  onMoveOutput: (id: string, direction: 'up' | 'down') => void;
}

/** Live-drag send interval; native volume calls take ~1-4ms. */
const VOLUME_THROTTLE_MS = 30;

/** System volume slider with independently expandable output controls. */
export default function VolumeControl({
  value,
  onChange,
  onRefresh = () => {},
  outputState,
  expanded,
  onToggleExpanded,
  updatingDeviceId,
  onSelectOutput,
  onRenameOutput,
  onMoveOutput,
}: VolumeControlProps) {
  const [editingDeviceId, setEditingDeviceId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelEditingRef = useRef(false);
  const selectedDevice = outputState?.devices.find(
    (device) => device.id === outputState.selectedDeviceId,
  );
  const enabledOutputCount =
    outputState?.devices.filter((device) => device.state === 'enabled').length ?? 0;
  const visibleDevices = outputState?.devices.filter((device) => device.state !== 'hidden') ?? [];

  /** Enters inline alias editing for one audio output. */
  const startEditing = (device: AudioOutputDevice) => {
    cancelEditingRef.current = false;
    setEditingDeviceId(device.id);
    setEditName(device.name);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  /** Commits an alias change; empty input clears the saved alias. */
  const finishEditing = (device: AudioOutputDevice) => {
    setEditingDeviceId(null);
    if (cancelEditingRef.current) {
      cancelEditingRef.current = false;
      setEditName(device.name);
      return;
    }
    const trimmed = editName.trim();
    if (trimmed === device.name) return;
    onRenameOutput(device.id, trimmed);
  };

  /** Handles commit/cancel keys without submitting twice through blur. */
  const handleEditKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter') {
      event.currentTarget.blur();
    } else if (event.key === 'Escape') {
      cancelEditingRef.current = true;
      event.currentTarget.blur();
    }
  };

  return (
    <div className='volume-section'>
      <div className='section-label-row'>
        <RefreshLabel
          className='audio-output-active-name'
          text={`All Speakers (${enabledOutputCount})${
            selectedDevice ? ` - ${selectedDevice.name || selectedDevice.originalName}` : ''
          }`}
          onRefresh={onRefresh}
        />
        <button
          className='section-toggle'
          onClick={onToggleExpanded}
          aria-expanded={expanded}
          aria-controls='audio-output-list'
          title={expanded ? 'Hide output speakers' : 'Show output speakers'}>
          <span className={`chevron${expanded ? ' expanded' : ''}`}>
            <Icon name='chevronRight' size={14} />
          </span>
        </button>
      </div>
      <Slider
        label='System volume'
        icon={<Icon name={speakerIconFor(value)} />}
        iconLabel={value === 0 ? 'Unmute system volume' : 'Mute system volume'}
        value={value}
        onChange={onChange}
        throttleMs={VOLUME_THROTTLE_MS}
        onIconClick={() => onChange(value > 0 ? 0 : 100)}
      />
      {expanded && outputState && (
        <div className='audio-output-list' id='audio-output-list'>
          {visibleDevices.length === 0 ? (
            <span className='audio-output-empty'>No audio outputs found</span>
          ) : (
            <>
              {visibleDevices.map((device) => {
                const selectionDisabled = updatingDeviceId !== null || device.state !== 'enabled';
                return (
                  <div className='audio-output-row' data-state={device.state} key={device.id}>
                    <label
                      className='audio-output-selector'
                      data-disabled={selectionDisabled}
                      title={
                        device.state === 'enabled'
                          ? `Select ${device.name}`
                          : `${device.name} is ${device.state}`
                      }>
                      <input
                        type='radio'
                        name='audio-output-device'
                        aria-label={`Select ${device.name}`}
                        checked={device.id === outputState.selectedDeviceId}
                        disabled={selectionDisabled}
                        onChange={() => onSelectOutput(device.id)}
                      />
                    </label>
                    {editingDeviceId === device.id ? (
                      <input
                        ref={inputRef}
                        className='monitor-name-input audio-output-name-input'
                        value={editName}
                        placeholder={device.originalName}
                        onChange={(event) => setEditName(event.target.value)}
                        onBlur={() => finishEditing(device)}
                        onKeyDown={handleEditKeyDown}
                      />
                    ) : (
                      <button
                        className='monitor-name audio-output-name'
                        onClick={() => startEditing(device)}
                        title={`Rename ${device.originalName}`}>
                        {device.name || device.originalName}
                      </button>
                    )}
                    <div className='monitor-reorder-buttons'>
                      <button
                        className='monitor-reorder-btn'
                        disabled={visibleDevices.indexOf(device) === 0}
                        onClick={() => onMoveOutput(device.id, 'up')}
                        title={`Move ${device.name} up`}>
                        <Icon name='chevronUp' size={12} />
                      </button>
                      <button
                        className='monitor-reorder-btn'
                        disabled={visibleDevices.indexOf(device) === visibleDevices.length - 1}
                        onClick={() => onMoveOutput(device.id, 'down')}
                        title={`Move ${device.name} down`}>
                        <Icon name='chevronDown' size={12} />
                      </button>
                    </div>
                  </div>
                );
              })}
            </>
          )}
        </div>
      )}
    </div>
  );
}
