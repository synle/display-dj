import DarkModeIcon from '@mui/icons-material/DarkMode';
import LightModeIcon from '@mui/icons-material/LightMode';
import Backdrop from '@mui/material/Backdrop';
import Button from '@mui/material/Button';
import ToggleButton from '@mui/material/ToggleButton';
import ToggleButtonGroup from '@mui/material/ToggleButtonGroup';
import Typography from '@mui/material/Typography';
import { useEffect, useState } from 'react';
import { Loading } from 'src/renderer/components/Loading';
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
    <>
      <div className='field'>
        <ToggleButtonGroup
          color='primary'
          size='small'
          fullWidth
          exclusive
          disabled={disabled}
          value={darkModeVal}
          onChange={(_e, val: string) => {
            if (val === null) {
              onToggleDarkMode(darkMode);
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
      </div>
      {/* backdrop */}
      <Backdrop
        sx={{
          background: (theme) => theme.palette.background.paper,
          color: (theme) => theme.palette.text.primary,
          zIndex: (theme) => theme.zIndex.drawer + 1,
        }}
        open={disabled}>
        <Typography variant='h6' className='flexAlignItems'>
          <Loading style={{ marginRight: '10px' }} />
          <strong>Updating dark mode...</strong>
        </Typography>
      </Backdrop>
    </>
  );
}
