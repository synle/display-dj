import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { Monitor } from 'src/types.d';

type MonitorBrightnessSettingFormProps = {
  monitors: Monitor[];
};

export function MonitorBrightnessSettingForm(props: MonitorBrightnessSettingFormProps) {
  const { monitors } = props;
  return (
    <>
      {monitors
        .filter((monitor) => !monitor.disabled)
        .map((monitor, idx) => (
          <MonitorBrightnessSetting key={`${monitor.id}-${idx}`} monitor={monitor} idx={idx + 1} />
        ))}
    </>
  );
}
