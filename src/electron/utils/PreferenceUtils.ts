import StorageUtils, { PREFERENCE_FILE_PATH } from 'src/electron/utils/StorageUtils';
import { Preference } from 'src/types.d';

const DEFAULT_PREFERENCES: Preference = {
  ddcctlBinary: `/usr/local/bin/ddcctl`,
  showIndividualDisplays: false,
};

const PreferenceUtils = {
  get: async () => {
    let preference: Preference = DEFAULT_PREFERENCES;

    try {
      preference = StorageUtils.readJSON(PREFERENCE_FILE_PATH, DEFAULT_PREFERENCES);
    } catch (err) {}

    // merging config keys
    for(const prefKey of Object.keys(DEFAULT_PREFERENCES)){
      preference[prefKey] = preference[prefKey] || DEFAULT_PREFERENCES[prefKey];
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
