import VolumeMuteIcon from '@mui/icons-material/VolumeMute';
import VolumeUpIcon from '@mui/icons-material/VolumeUp';
import { useEffect, useState, useMemo } from 'react';
import { VolumeSetting } from 'src/renderer/components/VolumeSetting';
import { Volume } from 'src/types.d';

type TimeOfDayProfileSettingsProps = {
};

type TimeOfDayFormMode = 'read' | 'edit';

const INTERVAL_TIME_MS = 1000;

export function TimeOfDayProfileSettings(props: TimeOfDayProfileSettingsProps) {
  const [mode, setMode] = useState<TimeOfDayFormMode>('read');

  switch(mode){
    case 'read':
      return <div className='TimeOfDayProfileSettings'>
    <span onClick={() => {setMode('edit')}}><TimeOfDayDisplay /></span>
  </div>;

    case 'edit':
      return <div className='TimeOfDayProfileSettings'>
      <TimeOfDayForm />
    </div>;
  }
}


function TimeOfDayDisplay(){
  const [time, setTime] = useState('');

  useEffect(
    () => {
      setInterval(_updateTime, INTERVAL_TIME_MS)
      _updateTime();

      function _updateTime(){
        setTime(new Date().toLocaleTimeString());
      }
    },
    []
  )

  return <>
    {time}
  </>
}


function TimeOfDayForm(){
  const [enabled, setEnabled] = useState(true);
  const [from, setFrom] = useState('17');
  const [to, setTo] = useState('8');

  return <>
    <div>Turn dark mode on based on schedule</div>
    <div>
    {
      enabled
        ? <>
            <input type='checkbox' checked={enabled} onChange={(e) => {setEnabled(e.target.checked)}}/>
            <span>From: </span>
            <TimeDropDown />
            <span>To: </span>
            <TimeDropDown />
          </>
        : <>
            <input type='checkbox' checked={enabled} onChange={(e) => {setEnabled(e.target.checked)}}/>
          </>
    }

    </div>
  </>
}


type TimeDropDownProps = {
  value?: string;
}

function TimeDropDown(props: TimeDropDownProps){
  return <select>
    <option value='1'>1 AM</option>
    <option value='2'>2 AM</option>
    <option value='3'>3 AM</option>
    <option value='4'>4 AM</option>
    <option value='5'>5 AM</option>
    <option value='6'>6 AM</option>
    <option value='7'>7 AM</option>
    <option value='8'>8 AM</option>
    <option value='9'>9 AM</option>
    <option value='10'>10 AM</option>
    <option value='11'>11 AM</option>
    <option value='12'>12 PM</option>
    <option value='13'>1 PM</option>
    <option value='14'>2 PM</option>
    <option value='15'>3 PM</option>
    <option value='16'>4 PM</option>
    <option value='17'>5 PM</option>
    <option value='18'>6 PM</option>
    <option value='19'>7 PM</option>
    <option value='20'>8 PM</option>
    <option value='21'>9 PM</option>
    <option value='22'>10 PM</option>
    <option value='23'>11 PM</option>
    <option value='24'>12 AM</option>
  </select>
}
