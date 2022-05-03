import React, { useEffect, useState } from 'react';
import { MonitorBrightnessSetting } from 'src/renderer/components/MonitorBrightnessSetting';
import { useUpdateMonitor } from 'src/renderer/hooks';
import { Monitor } from 'src/types.d';

type MonitorBrightnessSettingProps = {
  monitor: Monitor;
  idx: number;
};

export function MonitorNameInput(props: MonitorBrightnessSettingProps) {
  const [mode, setMode] = useState<string>('mode/read');
  const [name, setName] = useState('');
  const { monitor } = props;
  const { mutateAsync: updateMonitor } = useUpdateMonitor();

  const isSavingName = mode === 'mode/saving';

  const onDisplayNameChange = async (e: React.SyntheticEvent) => {
    e.preventDefault();
    let nameToUse = (name || '').trim() || `Monitor #${props.idx}`;

    if(nameToUse === monitor.name){
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

  return mode !== 'mode/read' ? (
    <form onSubmit={onDisplayNameChange}>
      <input
        className='field__value'
        value={name}
        placeholder='Enter a display name'
        autoFocus={true}
        onInput={(e) => setName((e.target as HTMLInputElement).value)}
        onBlur={onDisplayNameChange}
        required
        disabled={isSavingName}
        type='text'
      />
    </form>
  ) : (
    <div className='field__value field__value-readonly'>
      <a onClick={() => setMode('mode/edit')} title='Monitor Name' href='#'>
        {monitor.name}
      </a>
    </div>
  );
}
