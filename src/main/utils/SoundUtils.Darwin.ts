// @ts-nocheck
import osascript from 'node-osascript';

function executeOsaScript(command: string) {
  return new Promise((resolve, reject) => {
    osascript.execute(command, function (err: any, result: any) {
      if (err) return reject(err);
      resolve(result);
    });
  });
}

const SoundUtils = {
  getVolume: async () => {
    let command = `get volume settings`;

    try {
      const volume = await executeOsaScript(command).then((resp) => resp['input volume']);

      if (volume === 0) {
        return {
          value: 0,
          muted: true,
        };
      }

      return {
        value: volume,
        muted: false,
      };
    } catch (err) {
      console.error('SoundUtils.getVolume', command, err);
    }

    return {
      value: 50,
      muted: true,
    };
  },
  setVolume: async (value: number) => {
    let command = '';
    try {
      command = `set volume ${Math.floor(value / 10)}`;
      console.debug('SoundUtils.setVolume', command);
      await executeOsaScript(command);
    } catch (err) {
      console.error('SoundUtils.setMuted', value, command, err);
    }
  },
  setMuted: async (muted: boolean) => {
    let command = '';
    try {
      if (muted) {
        command = `set volume 0`;
      }
      console.debug('SoundUtils.setMuted', command);
      await executeOsaScript(command);
    } catch (err) {
      console.error('SoundUtils.setMuted', muted, command, err);
    }
  },
};

export default SoundUtils;
