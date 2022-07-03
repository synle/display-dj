import loudness from 'loudness';

const SoundUtils =  {
  getVolume: async () => {
    return {
      value: 50,
      muted: true,
    };
  },
  setVolume: async (value: number) => executeBash(`osascript -e "set Volume ${Math.floor(value / 10)}"`),
  setMuted: async (muted: boolean) => executeBash(`osascript -e "set Volume 0"`),
};

export default SoundUtils;
