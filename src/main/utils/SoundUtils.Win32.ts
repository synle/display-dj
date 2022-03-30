import { executeBash } from 'src/main/utils/ShellUtils';
import path from 'path';

const _getVolumeHelperBinary = async () => path.join(process['resourcesPath'], `win32_volume_helper.exe`);

const SoundUtils = {
  getVolume: async() => {
    const splits = (await executeBash(await _getVolumeHelperBinary())).split(/[ ]+/).map( s=> s.trim()).filter(s => s);
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
  setVolume: async (value: number) => await executeBash(`${await _getVolumeHelperBinary()} ${value}`),
  setMuted: async (muted: boolean) => await executeBash(`${await _getVolumeHelperBinary()} ${muted ? 'mute' : 'unmute'}`),
}

export default SoundUtils;
