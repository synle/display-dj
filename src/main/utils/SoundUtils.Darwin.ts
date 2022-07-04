import { executeBash } from 'src/main/utils/ShellUtils';

const SoundUtils =  {
  getVolume: async () => {
    let stdout= '';
    try{
      stdout = await executeBash(`osascript -e 'get volume settings'`);
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
      console.error('SoundUtils.getVolume', stdout, err);
    }

    return {
      value: 50,
      muted: true,
    };
  },
  setVolume: async (value: number) => {
    let command = '';
    try {
     command = `osascript -e "set Volume ${Math.floor(value / 10)}"`;
      console.debug('SoundUtils.setVolume', command);
      await executeBash(command)
    } catch(err){
      console.error('SoundUtils.setMuted',value,stdout, err);
    }
  },
  setMuted: async (muted: boolean)  => {
    let command = '';
    try {
      if(muted){
        command = `osascript -e "set Volume 0"`
      }
      console.debug('SoundUtils.setMuted', command);
      await executeBash(command)
    }catch(err){
      console.error('SoundUtils.setMuted',muted, stdout, err);
    }
  },
};

export default SoundUtils;
