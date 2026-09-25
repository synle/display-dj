import { useRef, useState } from 'react';
import { AudioOutputDevice, AudioOutputDeviceState, AudioOutputState } from '../types';
import Dropdown from './Dropdown';
import Slider from './Slider';

interface VolumeControlProps {
  value: number;
  onChange: (value: number) => void;
  outputState: AudioOutputState | null;
  expanded: boolean;
  onToggleExpanded: () => void;
  updatingDeviceId: string | null;
  onSelectOutput: (id: string) => void;
  onRenameOutput: (id: string, label: string) => void;
  onSetOutputState: (id: string, state: AudioOutputDeviceState) => void;
}

/** System volume slider with independently expandable output controls. */
export default function VolumeControl({
  value,
  onChange,
  outputState,
  expanded,
  onToggleExpanded,
  updatingDeviceId,
  onSelectOutput,
  onRenameOutput,
  onSetOutputState,
}: VolumeControlProps) {
  const [editingDeviceId, setEditingDeviceId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const [showHiddenOutputs, setShowHiddenOutputs] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelEditingRef = useRef(false);
  const selectedDevice = outputState?.devices.find(
    (device) => device.id === outputState.selectedDeviceId,
  );
  const enabledOutputCount =
    outputState?.devices.filter((device) => device.state === 'enabled').length ?? 0;
  const hiddenOutputCount =
    outputState?.devices.filter(
      (device) => device.id !== outputState.selectedDeviceId && device.state === 'hidden',
    ).length ?? 0;
  const visibleDevices =
    outputState?.devices.filter(
      (device) =>
        device.id === outputState.selectedDeviceId ||
        showHiddenOutputs ||
        device.state !== 'hidden',
    ) ?? [];

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
        <span className='section-label audio-output-active-name'>
          {`All Speakers (${enabledOutputCount})`}
          {!expanded && selectedDevice
            ? ` - ${selectedDevice.name || selectedDevice.originalName}`
            : ''}
        </span>
        <button
          className='section-toggle'
          onClick={onToggleExpanded}
          title={expanded ? 'Hide output speakers' : 'Show output speakers'}>
          <span className={`chevron${expanded ? ' expanded' : ''}`}>&#9662;</span>
        </button>
      </div>
      <Slider
        icon={value === 0 ? '\uD83D\uDD07' : '\uD83D\uDD0A'}
        value={value}
        onChange={onChange}
        onIconClick={() => onChange(value > 0 ? 0 : 100)}
      />
      {expanded && outputState && (
        <div className='audio-output-list'>
          {visibleDevices.length === 0 && hiddenOutputCount === 0 ? (
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
                    <Dropdown
                      className='audio-output-state'
                      aria-label={`State for ${device.name}`}
                      value={device.state}
                      disabled={updatingDeviceId !== null}
                      onChange={(event) =>
                        onSetOutputState(device.id, event.target.value as AudioOutputDeviceState)
                      }>
                      <option value='enabled'>Enabled</option>
                      <option value='disabled'>Disabled</option>
                      <option value='hidden'>Hidden</option>
                    </Dropdown>
                  </div>
                );
              })}
              {hiddenOutputCount > 0 && (
                <button
                  className='audio-output-hidden-toggle'
                  onClick={() => setShowHiddenOutputs((current) => !current)}>
                  {showHiddenOutputs
                    ? 'Hide hidden outputs'
                    : `Show hidden outputs (${hiddenOutputCount})`}
                </button>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}
