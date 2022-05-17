import { useEffect, useState } from 'react';
import { useToggleDarkMode } from 'src/renderer/hooks';
import DarkModeSvg from 'src/renderer/svg/darkMode.svg';
import LightModeSvg from 'src/renderer/svg/lightMode.svg';
import Button from '@mui/material/Button';
import ToggleButtonGroup from '@mui/material/ToggleButtonGroup';
import ToggleButton from '@mui/material/ToggleButton';
import DarkModeIcon from '@mui/icons-material/DarkMode';
import LightModeIcon from '@mui/icons-material/LightMode';
import Typography from '@mui/material/Typography';

type DarkModeSettingFormProps = {
  darkMode: boolean;
};

export function DarkModeSettingForm(props: DarkModeSettingFormProps) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState<boolean>(darkModeFromProps);
  const { mutateAsync: updateDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async (newDarkMode: boolean | undefined) => {
    if(newDarkMode === undefined){
      newDarkMode = !darkMode;
    }

    setDarkMode(newDarkMode);

    await updateDarkMode(newDarkMode);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  const darkModeVal = darkMode ? '1': '0';

  return (
    <ToggleButtonGroup
      color="primary"
      size='small'
      fullWidth
      exclusive
      value={darkModeVal}
      onChange={(_e, val: string) => onToggleDarkMode(val === '1')}
    >
      <ToggleButton value="1"><DarkModeIcon /> <Typography variant='subtitle1' sx={{ml: 1}}>Dark</Typography></ToggleButton>
      <ToggleButton value="0"><LightModeIcon /> <Typography variant='subtitle1' sx={{ml: 1}}>Light</Typography></ToggleButton>
    </ToggleButtonGroup>
  );
}
