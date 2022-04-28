import { useEffect, useState } from 'react';
import { useToggleDarkMode } from 'src/renderer/hooks';
import DarkModeSvg from 'src/renderer/svg/darkMode.svg';
import LightModeSvg from 'src/renderer/svg/lightMode.svg';

type DarkModeSettingFormProps = {
  darkMode: boolean;
};

export function DarkModeSettingForm(props: DarkModeSettingFormProps) {
  const darkModeFromProps = props.darkMode === true;
  const [darkMode, setDarkMode] = useState<boolean>(darkModeFromProps);
  const { mutateAsync: updateDarkMode } = useToggleDarkMode();

  const onToggleDarkMode = async () => {
    const newDarkMode = !darkMode;

    setDarkMode(newDarkMode);

    await updateDarkMode(newDarkMode);
  };

  useEffect(() => {
    setDarkMode(darkModeFromProps);
  }, [darkModeFromProps]);

  return (
    <button className='btnToggleDarkMode' onClick={onToggleDarkMode} title='Toggle Dark Mode'>
      {darkMode ? (
        <>
          <DarkModeSvg /> <span>Mode: Dark</span>
        </>
      ) : (
        <>
          <LightModeSvg />
          <span>Mode: Light</span>
        </>
      )}
    </button>
  );
}
