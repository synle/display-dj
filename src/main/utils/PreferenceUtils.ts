import StorageUtils, { PREFERENCE_FILE_PATH } from 'src/main/utils/StorageUtils';
import { Preference } from 'src/types.d';

const MIN_BRIGHTNESS = 5;// NOTE : here if we set it to 0, some monitors / laptops will turn off the backlit entirely and make it unusable...

const DEFAULT_PREFERENCES: Preference = {
  showIndividualDisplays: false,
  logging: false,
  brightnessDelta: 25,
  brightnessPresets: [
    { level: MIN_BRIGHTNESS },
    { level: 50 },
    { level: 100 },
  ],
  volumePresets: [
    { level: 0 },
    { level: 50 },
    { level: 100 },
  ],
  keyBindings: [
    { key: `Shift+Escape`, command: [`command/changeDarkMode/toggle`] },
    {
      key: `Shift+F1`,
      command: [`command/changeDarkMode/dark`, `command/changeBrightness/${MIN_BRIGHTNESS}`],
      notification: `Switching to Dark Profile`,
    },
    {
      key: `Shift+F2`,
      command: [`command/changeDarkMode/light`, `command/changeBrightness/100`],
      notification: `Switching to Light Profile`,
    },
    { key: `Shift+F3`, command: [`command/changeBrightness/${MIN_BRIGHTNESS}`], notification: `Brightness is ${MIN_BRIGHTNESS}%`,},
    { key: `Shift+F4`, command: [`command/changeBrightness/50`],notification: `Brightness is 50%`, },
    { key: `Shift+F5`, command: [`command/changeBrightness/100`], notification: `Brightness is 100%`,},
    { key: `Shift+F6`, command: [`command/changeVolume/0`],notification: `Volume is Muted`, },
    { key: `Shift+F7`, command: [`command/changeVolume/100`],notification: `Volume is 100%`, },
  ],
};

const PreferenceUtils = {
  get: async () => {
    let preference: Preference = DEFAULT_PREFERENCES;
    let shouldSync = false;

    preference = StorageUtils.readJSON(PREFERENCE_FILE_PATH);

    if (preference === undefined) {
      shouldSync = true; // sync the default value
      preference = DEFAULT_PREFERENCES; // fallback if preference is empty
    }

    // merging config keys
    for (const prefKey of Object.keys(DEFAULT_PREFERENCES)) {
      //@ts-ignore
      if (preference[prefKey] === undefined) {
        shouldSync = true; // when a key is missing, then also sync it up

        // @ts-ignore
        preference[prefKey] = DEFAULT_PREFERENCES[prefKey];
      }
    }

    if (shouldSync) {
      // essentially we want to sync if the keys mismatched
      StorageUtils.writeJSON(PREFERENCE_FILE_PATH, preference);
    }

    return preference;
  },
  patch: async (newPartialPrefs: Preference) => {
    const oldPrefs = await PreferenceUtils.get();
    const newPrefs = {
      ...oldPrefs,
      ...newPartialPrefs,
    };

    StorageUtils.writeJSON(PREFERENCE_FILE_PATH, newPrefs);

    return newPrefs;
  },
  getKeybindings: async () => {
    return (await PreferenceUtils.get()).keyBindings;
  },
  getBrightnessPresets: async() => {
    return (await PreferenceUtils.get()).brightnessPresets;
  },
  getVolumePresets: async() => {
    return (await PreferenceUtils.get()).volumePresets;
  }
};

export default PreferenceUtils;
