import { executeOsaScript } from 'src/main/utils/ShellUtils';
// /usr/bin/osascript
const SoundUtils =  {
  getVolume: async () => {
    let stdout= '';
    let command = `get volume settings`
    try{
      stdout = await executeOsaScript(command);
      stdout = stdout.substr(stdout.indexOf(`output volume:`) + `output volume:`.length)

      const volume = parseInt(stdout.substr(0, stdout.indexOf(`,`)));

      if(volume === 0){
        return {
          value: 0,
          muted: true
        }
      }

      return {
          value: volume,
          muted: false
        }
    }catch(err){
      console.error('SoundUtils.getVolume',command, stdout, err);
    }

    return {
      value: 50,
      muted: true,
    };
  },
  setVolume: async (value: number) => {
    let command = '';
    try {
     command = `set Volume ${Math.floor(value / 10)}`;
      console.debug('SoundUtils.setVolume', command);
      await executeOsaScript(command)
    } catch(err){
      console.error('SoundUtils.setMuted',value,command, err);
    }
  },
  setMuted: async (muted: boolean)  => {
    let command = '';
    try {
      if(muted){
        command = `set Volume 0`
      }
      console.debug('SoundUtils.setMuted', command);
      await executeOsaScript(command)
    }catch(err){
      console.error('SoundUtils.setMuted',muted, command, err);
    }
  },
};

export default SoundUtils;
