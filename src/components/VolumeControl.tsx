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

type EditLocation = 'active' | 'list';

/** System volume slider with editable active-output label and expanded endpoint controls. */
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
  const [editingLocation, setEditingLocation] = useState<EditLocation | null>(null);
  const [editName, setEditName] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  const cancelEditingRef = useRef(false);
  const selectedDevice = outputState?.devices.find(
    (device) => device.id === outputState.selectedDeviceId,
  );

  /** Enters inline alias editing for one audio output. */
  const startEditing = (device: AudioOutputDevice, location: EditLocation) => {
    cancelEditingRef.current = false;
    setEditingDeviceId(device.id);
    setEditingLocation(location);
    setEditName(device.name);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  /** Commits an alias change; empty input clears the saved alias. */
  const finishEditing = (device: AudioOutputDevice) => {
    setEditingDeviceId(null);
    setEditingLocation(null);
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
      {selectedDevice ? (
        editingDeviceId === selectedDevice.id && editingLocation === 'active' ? (
          <input
            ref={inputRef}
            className='monitor-name-input audio-output-active-name-input'
            value={editName}
            placeholder={selectedDevice.originalName}
            onChange={(event) => setEditName(event.target.value)}
            onBlur={() => finishEditing(selectedDevice)}
            onKeyDown={handleEditKeyDown}
          />
        ) : (
          <button
            className='section-label audio-output-active-name'
            onClick={() => startEditing(selectedDevice, 'active')}
            title={`Rename active output ${selectedDevice.originalName}`}>
            {selectedDevice.name || selectedDevice.originalName}
          </button>
        )
      ) : (
        <span className='section-label audio-output-active-name'>Audio Output</span>
      )}
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
                <label className='audio-output-selector' data-disabled={selectingDeviceId !== null}>
                  <input
                    type='radio'
                    name='audio-output-device'
                    aria-label={`Select ${device.name}`}
                    checked={device.id === outputState.selectedDeviceId}
                    disabled={selectingDeviceId !== null}
                    onChange={() => onSelectOutput(device.id)}
                  />
                </label>
                {editingDeviceId === device.id && editingLocation === 'list' ? (
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
                    onClick={() => startEditing(device, 'list')}
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
