import Slider from "./Slider";

interface AllMonitorsControlProps {
  brightness: number;
  onBrightnessChange: (value: number) => void;
  monitorCount: number;
  minBrightness: number;
}

export default function AllMonitorsControl({
  brightness,
  onBrightnessChange,
  monitorCount,
  minBrightness,
}: AllMonitorsControlProps) {
  return (
    <div className="all-monitors-section">
      <div className="section-label">All Monitors ({monitorCount})</div>
      <Slider
        icon="☀"
        value={brightness}
        min={minBrightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(brightness > minBrightness ? minBrightness : 100)}
      />
    </div>
  );
}
