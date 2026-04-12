import Slider from "./Slider";

interface AllMonitorsControlProps {
  brightness: number;
  contrast: number;
  showContrast: boolean;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
}

export default function AllMonitorsControl({
  brightness,
  contrast,
  showContrast,
  onBrightnessChange,
  onContrastChange,
}: AllMonitorsControlProps) {
  return (
    <div className="all-monitors-section">
      <div className="section-label">All Monitors</div>
      <Slider icon="☀" value={brightness} onChange={onBrightnessChange} />
      {showContrast && (
        <Slider icon="◑" value={contrast} onChange={onContrastChange} />
      )}
    </div>
  );
}
