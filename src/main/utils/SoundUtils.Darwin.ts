import loudness from 'loudness';

const SoundUtils = {
  getVolume: async() => {
    return {
      value: await loudness.getVolume(),
      muted: await loudness.getMuted()
    }
  },
  setVolume: async (value: number) => loudness.setVolume(value),
  setMuted: async (muted: boolean) => loudness.setMuted(muted),
}

export default SoundUtils;