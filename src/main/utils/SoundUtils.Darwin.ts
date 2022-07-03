import loudness from 'loudness';

const IntelMacSoundUtils = {
  getVolume: async () => {
    return {
      value: await loudness.getVolume(),
      muted: await loudness.getMuted(),
    };
  },
  setVolume: async (value: number) => loudness.setVolume(value),
  setMuted: async (muted: boolean) => loudness.setMuted(muted),
};

const M1MacSoundUtils = {
  getVolume: async () => {
    return {
      value: await loudness.getVolume(),
      muted: await loudness.getMuted(),
    };
  },
  setVolume: async (value: number) => loudness.setVolume(value),
  setMuted: async (muted: boolean) => loudness.setMuted(muted),
};

// export default M1MacSoundUtils;
export default IntelMacSoundUtils;

// osascript -e "set Volume 0"
