import loudness from 'loudness';

const SoundUtils = {
  getVolume: async () => loudness.getVolume(),
  setVolume: async (value: number) => loudness.setVolume(value),
  getMuted: async () => loudness.getMuted(),
  setMuted: async (muted: boolean) => loudness.setMuted(muted),
}

export default SoundUtils;
