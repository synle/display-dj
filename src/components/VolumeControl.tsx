import { useRef, useState } from 'react';
import { AudioOutputDevice, AudioOutputState } from '../types';
import Slider from './Slider';

interface VolumeControlProps {
  value: number;
  onChange: (value: number) => void;
  outputState: AudioOutputState | null;
  expanded: boolean;
  selectingDeviceId: string | null;
  onSelectOutput: (id: string) => void;
  onRenameOutput: (id: string, label: string) => void;
}

/** System volume slider with active-output label and expanded endpoint controls. */
export default function VolumeControl({
  value,
  onChange,
  outputState,
  expanded,
  selectingDeviceId,
  onSelectOutput,
  onRenameOutput,
}: VolumeControlProps) {
  const [editingDeviceId, setEditingDeviceId] = useState<string | null>(null);
  const [editName, setEditName] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelEditingRef = useRef(false);
  const selectedDevice = outputState?.devices.find(
    (device) => device.id === outputState.selectedDeviceId,
  );

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
      <span className='section-label'>{selectedDevice?.name ?? 'Audio Output'}</span>
      <Slider
        icon={value === 0 ? '\uD83D\uDD07' : '\uD83D\uDD0A'}
        value={value}
        onChange={onChange}
        onIconClick={() => onChange(value > 0 ? 0 : 100)}
      />
      {expanded && outputState && (
        <div className='audio-output-list'>
          {outputState.devices.length === 0 ? (
            <span className='audio-output-empty'>No audio outputs found</span>
          ) : (
            outputState.devices.map((device) => (
              <div className='audio-output-row' key={device.id}>
                <input
                  type='radio'
                  name='audio-output-device'
                  aria-label={`Select ${device.name}`}
                  checked={device.id === outputState.selectedDeviceId}
                  disabled={selectingDeviceId !== null}
                  onChange={() => onSelectOutput(device.id)}
                />
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
              </div>
            ))
          )}
        </div>
      )}
    </div>
  );
}
