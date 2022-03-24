import StorageUtils, { PREFERENCE_FILE_PATH } from 'src/electron/utils/StorageUtils';
import { Preference } from 'src/types.d';

const DEFAULT_PREFERENCES: Preference = {
  ddcctlBinary: `/usr/local/bin/ddcctl`,
  brightnessBinary: '/usr/local/bin/brightness',
  showIndividualDisplays: false,
  brightnessDelta: 50,
  brightnessPresets: [
    { level: 0 },
    { level: 50 },
    { level: 100 },
  ],
  keyBindings: [
    { key: 'Shift+Escape', command: ['command/changeDarkMode/toggle'] },
    {
      key: 'Shift+F1',
      command: ['command/changeDarkMode/dark', 'command/changeBrightness/10'],
    },
    {
      key: 'Shift+F2',
      command: ['command/changeDarkMode/light', 'command/changeBrightness/100'],
    },
    { key: 'Shift+F3', command: ['command/changeBrightness/0'] },
    { key: 'Shift+F4', command: ['command/changeBrightness/50'] },
    { key: 'Shift+F5', command: ['command/changeBrightness/100'] },
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

    StorageUtils.writeJSON(PREFERENCE_FILE_PATH, {
      ...oldPrefs,
      ...newPartialPrefs,
    });
  },
  getKeybindings: async () => {
    return (await PreferenceUtils.get()).keyBindings;
  },
};

export default PreferenceUtils;
