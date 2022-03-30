import { executeBash } from 'src/main/utils/ShellUtils';
import path from 'path';

const binaryPath = process.env.APP_MODE === 'dev'
  ? path.join(process.cwd(), `src/binaries/win32_volume_helper.exe`)
  : path.join(process['resourcesPath'], `resources/win32_volume_helper.exe`);

const SoundUtils = {
  getVolume: async() => {
    const splits = (await executeBash(binaryPath)).split(/[ ]+/).map( s=> s.trim()).filter(s => s);
    const value: number = parseInt(splits[0]);
    const muted: boolean = splits[1] === '1';

    if(value > 100 || value < 0){
      throw 'Failed to get the volume';
    }

    return {
      value,
      muted
    }
  },
  setVolume: async (value: number) => await executeBash(`${binaryPath} ${value}`),
  setMuted: async (muted: boolean) => await executeBash(`${binaryPath} ${muted ? 'mute' : 'unmute'}`),
}

export default SoundUtils;
