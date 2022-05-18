import React, { useEffect, useState } from 'react';
import { Loading } from 'src/renderer/components/Loading';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { useUpdateMonitor } from 'src/renderer/hooks';
import { Monitor } from 'src/types.d';
import Link from '@mui/material/Link';
import TextField from '@mui/material/TextField';
import Typography from '@mui/material/Typography';

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};

type InputMode = 'mode/read' | 'mode/saving' | 'mode/edit';

export function MonitorNameInput(props: MonitorBrightnessSettingProps) {
  const [mode, setMode] = useState<InputMode>('mode/read');
  const [name, setName] = useState('');
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();

  const isSavingName = mode === 'mode/saving';

  const onDisplayNameChange = async (e: React.SyntheticEvent) => {
    e.preventDefault();
    let nameToUse = (name || '').trim() || `Monitor #${props.idx}`;

    if (nameToUse === monitor.name) {
      // ignore if name is the same and not changed
      setMode('mode/read');
      return;
    }

    setName(nameToUse);
    setMode('mode/saving');

    await updateMonitor({
      id: monitor.id,
      name: nameToUse,
    });
  };

  useEffect(() => {
    setName(monitor.name);
    setMode('mode/read');
  }, [monitor.name]);
  switch (mode) {
    case 'mode/read':
      return (
        <Link onClick={() => setMode('mode/edit')} title='Monitor Name'>
          <strong>{monitor.name}</strong>
        </Link>
      );
    case 'mode/edit':
      return (
        <form onSubmit={onDisplayNameChange}>
          <TextField
            variant="outlined"
            size='small'
            fullWidth
            label='Enter a display name'
            placeholder='Enter a display name'
            value={name}
            autoFocus={true}
            onInput={(e) => setName((e.target as HTMLInputElement).value)}
            onBlur={onDisplayNameChange}
            required
            disabled={isSavingName} />
        </form>
      );

    case 'mode/saving':
      return (
        <Typography variant='subtitle1' className='flexAlignItems'>
          <Loading style={{ marginRight: '10px' }} />
          <span style={{ marginRight: '5px' }}>Saving</span>
          <strong>"{name}"</strong>
          <span>...</span>
        </Typography>
      );
  }
}
