import { useUpdatePreferences } from 'src/renderer/hooks';
import ToggleSvg from 'src/renderer/svg/toggle.svg';
import { Preference } from 'src/types.d';

type ToggleAllDisplayProps = {
  preference: Preference;
};

export function ToggleAllDisplay(props: ToggleAllDisplayProps) {
  const { preference } = props;
  const { mutateAsync: updatePreferences } = useUpdatePreferences();

  const onToggleAll = () => {
    preference.showIndividualDisplays = !preference.showIndividualDisplays;
    updatePreferences(preference);
  };

  return (
    <span
      className='iconBtn'
      onClick={onToggleAll}
      title={
        preference.showIndividualDisplays
          ? 'Collapse individual displays brightness'
          : 'Expand individual displays brightness'
      }>
      <ToggleSvg />
    </span>
  );
}
