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
      preference = StorageUtils.readJSON(PREFERENCE_FILE_PATH);
    } catch (err) {}

    // hooking up default value
    preference.ddcctlBinary = preference.ddcctlBinary || DEFAULT_PREFERENCES.ddcctlBinary;
    preference.showIndividualDisplays =
      preference.showIndividualDisplays || DEFAULT_PREFERENCES.showIndividualDisplays;

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
