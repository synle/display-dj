import DarkModeIcon from '@mui/icons-material/DarkMode';
import LightModeIcon from '@mui/icons-material/LightMode';
import Button from '@mui/material/Button';
import ToggleButton from '@mui/material/ToggleButton';
import ToggleButtonGroup from '@mui/material/ToggleButtonGroup';
import Typography from '@mui/material/Typography';
import { useEffect, useState } from 'react';
import { useToggleDarkMode } from 'src/renderer/hooks';

type DarkModeSettingFormProps = {
  darkMode: boolean;
};

export function DarkModeSettingForm(props: DarkModeSettingFormProps) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState<boolean>(darkModeFromProps);
  const [disabled, setDisabled] = useState(false);
  const { mutateAsync: updateDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async (newDarkMode: boolean) => {
    setDarkMode(newDarkMode);
    setDisabled(true);
    await updateDarkMode(newDarkMode);
    setDisabled(false);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  const darkModeVal = darkMode ? '1' : '0';

  return (
    <ToggleButtonGroup
      color='primary'
      size='small'
      fullWidth
      exclusive
      disabled={disabled}
      value={darkModeVal}
      onChange={(_e, val: string) => {
        if (val === null) {
          return;
        }
        onToggleDarkMode(val === '1');
      }}>
      <ToggleButton value='1'>
        <DarkModeIcon />{' '}
        <Typography variant='subtitle1' sx={{ ml: 1 }}>
          Dark
        </Typography>
      </ToggleButton>
      <ToggleButton value='0'>
        <LightModeIcon />{' '}
        <Typography variant='subtitle1' sx={{ ml: 1 }}>
          Light
        </Typography>
      </ToggleButton>
    </ToggleButtonGroup>
  );
}
