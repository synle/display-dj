import { useState, useRef } from "react";
import { Monitor } from "../types";
import Slider from "./Slider";

interface MonitorControlProps {
  monitor: Monitor;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
  onRename: (name: string) => void;
}

export default function MonitorControl({
  monitor,
  onBrightnessChange,
  onContrastChange,
  onRename,
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
    if (editName.trim() && editName.trim() !== monitor.name) {
      onRename(editName.trim());
    }
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
          onChange={(e) => setEditName(e.target.value)}
          onBlur={finishEditing}
          onKeyDown={handleKeyDown}
        />
      ) : (
        <button className="monitor-name" onClick={startEditing}>
          {monitor.name}
        </button>
      )}
      <Slider
        icon={monitor.isBuiltIn ? "\uD83D\uDCBB" : "\uD83D\uDDA5"}
        value={monitor.brightness}
        onChange={onBrightnessChange}
      />
      {monitor.supportsContrast && (
        <Slider icon="\u25D1" value={monitor.contrast} onChange={onContrastChange} />
      )}
    </div>
  );
}
