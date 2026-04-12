import Slider from "./Slider";

interface VolumeControlProps {
  value: number;
  onChange: (value: number) => void;
}

export default function VolumeControl({ value, onChange }: VolumeControlProps) {
  return (
    <div className="volume-section">
      <Slider
        icon={value === 0 ? "\uD83D\uDD07" : "\uD83D\uDD0A"}
        value={value}
        onChange={onChange}
        onIconClick={() => onChange(value > 0 ? 0 : 100)}
      />
    </div>
  );
}
