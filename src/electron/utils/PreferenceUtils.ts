import StorageUtils, { PREFERENCE_FILE_PATH } from 'src/electron/utils/StorageUtils';
import { Preference } from 'src/types.d';

const DEFAULT_PREFERENCES: Preference = {
  ddcctlBinary: `/usr/local/bin/ddcctl`,
  showIndividualDisplays: false,
  brightnessDelta: 50,
  keyBindings: [
    { key: 'Shift+F1', command: 'command/changeBrightness/Down' },
    { key: 'Shift+F2', command: 'command/changeBrightness/Up' },
    { key: 'Shift+F3', command: 'command/changeBrightness/0' },
    { key: 'Shift+F4', command: 'command/changeBrightness/50' },
    { key: 'Shift+F5', command: 'command/changeBrightness/100' },
  ],
};

const PreferenceUtils = {
  get: async () => {
    let preference: Preference = DEFAULT_PREFERENCES;
    let shouldSync = false;

    preference = StorageUtils.readJSON(PREFERENCE_FILE_PATH);

    if (preference === undefined) {
      preference = DEFAULT_PREFERENCES;
      shouldSync = true; // sync the default value
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
};

export default PreferenceUtils;
