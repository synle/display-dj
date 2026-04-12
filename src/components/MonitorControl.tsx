import { useState, useRef } from "react";
import { Monitor } from "../types";
import Slider from "./Slider";

interface MonitorControlProps {
  monitor: Monitor;
  onBrightnessChange: (value: number) => void;
  onRename: (name: string) => void;
  minBrightness: number;
}

export default function MonitorControl({
  monitor,
  onBrightnessChange,
  onRename,
  minBrightness,
}: MonitorControlProps) {
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(monitor.name);
  const inputRef = useRef<HTMLInputElement>(null);

  const startEditing = () => {
    setEditName(monitor.name);
    setEditing(true);
    setTimeout(() => inputRef.current?.focus(), 0);
  };

  const finishEditing = () => {
    setEditing(false);
    const trimmed = editName.trim();
    if (trimmed === monitor.name) return;
    // Empty input clears the custom name, reverting to the API default
    onRename(trimmed);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      finishEditing();
    } else if (e.key === "Escape") {
      setEditing(false);
      setEditName(monitor.name);
    }
  };

  return (
    <div className="monitor-control">
      {editing ? (
        <input
          ref={inputRef}
          className="monitor-name-input"
          value={editName}
          placeholder={monitor.originalName}
          onChange={(e) => setEditName(e.target.value)}
          onBlur={finishEditing}
          onKeyDown={handleKeyDown}
        />
      ) : (
        <button className="monitor-name" onClick={startEditing}>
          {monitor.name || monitor.originalName}
        </button>
      )}
      <Slider
        icon={monitor.isBuiltIn ? "\uD83D\uDCBB" : "\uD83D\uDDA5"}
        value={monitor.brightness}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(monitor.brightness > minBrightness ? minBrightness : 100)}
      />
    </div>
  );
}
