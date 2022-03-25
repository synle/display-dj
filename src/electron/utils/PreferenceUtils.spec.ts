import PreferenceUtils from 'src/electron/utils/PreferenceUtils';
import DisplayUtils from 'src/electron/utils/DisplayUtils';

describe('PreferenceUtils', () => {
  beforeAll(async() => {
    await DisplayUtils.reset();
  });

  test('get', async () => {
    const actual = await PreferenceUtils.get();
    expect(actual).toMatchInlineSnapshot();
  });

  test('getKeybindings', async () => {
    const actual = await PreferenceUtils.getKeybindings();
    expect(actual).toMatchInlineSnapshot();
  });

  test('getBrightnessPresets', async () => {
    const actual = await PreferenceUtils.getBrightnessPresets();
    expect(actual).toMatchInlineSnapshot();
  });

  test('patch', async () => {
    const oldPrefs = await PreferenceUtils.get();
    const actual = await PreferenceUtils.patch({
      ...oldPrefs,
      brightnessDelta: 30
    });
    expect(actual).toMatchInlineSnapshot();
  });
});
