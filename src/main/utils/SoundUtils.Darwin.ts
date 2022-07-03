const SoundUtils =  {
  getVolume: async () => {
    try{
      const stdout = await executeBash(`osascript -e 'get volume settings'`);
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
    }catch(err){}

    return {
      value: 50,
      muted: true,
    };
  },
  setVolume: async (value: number) => executeBash(`osascript -e "set Volume ${Math.floor(value / 10)}"`),
  setMuted: async (muted: boolean) => executeBash(`osascript -e "set Volume 0"`),
};

export default SoundUtils;
