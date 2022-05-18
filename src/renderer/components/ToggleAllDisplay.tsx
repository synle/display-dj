import ExpandLessIcon from '@mui/icons-material/ExpandLess';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import IconButton from '@mui/material/IconButton';
import { useUpdatePreferences } from 'src/renderer/hooks';
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
    <IconButton
      onClick={onToggleAll}
      title={
        preference.showIndividualDisplays
          ? 'Collapse and control all displays'
          : 'Expand and control individual displays / monitors'
      }>
      {preference.showIndividualDisplays ? <ExpandMoreIcon /> : <ExpandLessIcon />}
    </IconButton>
  );
}
