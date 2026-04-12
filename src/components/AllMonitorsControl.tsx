import Slider from "./Slider";

interface AllMonitorsControlProps {
  brightness: number;
  onBrightnessChange: (value: number) => void;
}

export default function AllMonitorsControl({
  brightness,
  onBrightnessChange,
}: AllMonitorsControlProps) {
  return (
    <div className="all-monitors-section">
      <div className="section-label">All Monitors</div>
      <Slider
        icon="☀"
        value={brightness}
        onChange={onBrightnessChange}
        onIconClick={() => onBrightnessChange(brightness > 0 ? 0 : 100)}
      />
    </div>
  );
}
