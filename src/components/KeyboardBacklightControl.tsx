import Slider from './Slider';

interface KeyboardBacklightControlProps {
  /** Current backlight level 0..100. Will be snapped to the nearest 25 by the slider. */
  value: number;
  /** Called with the new level (0/25/50/75/100). The backend snaps too, defensively. */
  onChange: (value: number) => void;
}

/**
 * Built-in laptop keyboard backlight slider (beta).
 *
 * Stepped at 25% so the only reachable values are 0/25/50/75/100 — matches the
 * same snap the backend applies to `command/changeKeyboardBacklight/{value}`
 * shortcut commands, so the slider and the hotkey produce identical hardware
 * state.
 *
 * Icon: U+2328 (⌨) keyboard symbol. Click toggles 0 ↔ 100 like the volume icon
 * mute/unmute toggle. Caller is responsible for hiding the component entirely
 * when the platform layer reports the device as unsupported or when
 * `keyboardBacklight.enabled` is false in preferences.
 */
export default function KeyboardBacklightControl({
  value,
  onChange,
}: KeyboardBacklightControlProps) {
  return (
    <div className='keyboard-backlight-section'>
      <Slider
        icon={'\u2328'}
        value={value}
        step={25}
        onChange={onChange}
        onIconClick={() => onChange(value > 0 ? 0 : 100)}
      />
    </div>
  );
}
