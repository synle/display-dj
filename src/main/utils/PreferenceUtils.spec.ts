import PreferenceUtils from 'src/main/utils/PreferenceUtils';
import DisplayUtils from 'src/main/utils/DisplayUtils';
import StorageUtils, {
  MONITOR_CONFIG_FILE_PATH,
  PREFERENCE_FILE_PATH,
} from 'src/main/utils/StorageUtils';

describe('PreferenceUtils', () => {
  beforeAll(async () => {
    await Promise.all([
      StorageUtils.delete(MONITOR_CONFIG_FILE_PATH),
      StorageUtils.delete(PREFERENCE_FILE_PATH),
    ]);
  });

  test('get', async () => {
    const actual = await PreferenceUtils.get();
    expect(actual).toMatchInlineSnapshot(`
      Object {
        "brightnessBinary": "/usr/local/bin/brightness",
        "brightnessDelta": 25,
        "brightnessPresets": Array [
          Object {
            "level": 0,
          },
          Object {
            "level": 50,
          },
          Object {
            "level": 100,
          },
        ],
        "ddcctlBinary": "/usr/local/bin/ddcctl",
        "keyBindings": Array [
          Object {
            "command": Array [
              "command/changeDarkMode/toggle",
            ],
            "key": "Shift+Escape",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/dark",
              "command/changeBrightness/10",
            ],
            "key": "Shift+F1",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/light",
              "command/changeBrightness/100",
            ],
            "key": "Shift+F2",
          },
          Object {
            "command": Array [
              "command/changeBrightness/0",
            ],
            "key": "Shift+F3",
          },
          Object {
            "command": Array [
              "command/changeBrightness/50",
            ],
            "key": "Shift+F4",
          },
          Object {
            "command": Array [
              "command/changeBrightness/100",
            ],
            "key": "Shift+F5",
          },
        ],
        "showIndividualDisplays": false,
      }
    `);
  });

  test('getKeybindings', async () => {
    const actual = await PreferenceUtils.getKeybindings();
    expect(actual).toMatchInlineSnapshot(`
      Array [
        Object {
          "command": Array [
            "command/changeDarkMode/toggle",
          ],
          "key": "Shift+Escape",
        },
        Object {
          "command": Array [
            "command/changeDarkMode/dark",
            "command/changeBrightness/10",
          ],
          "key": "Shift+F1",
        },
        Object {
          "command": Array [
            "command/changeDarkMode/light",
            "command/changeBrightness/100",
          ],
          "key": "Shift+F2",
        },
        Object {
          "command": Array [
            "command/changeBrightness/0",
          ],
          "key": "Shift+F3",
        },
        Object {
          "command": Array [
            "command/changeBrightness/50",
          ],
          "key": "Shift+F4",
        },
        Object {
          "command": Array [
            "command/changeBrightness/100",
          ],
          "key": "Shift+F5",
        },
      ]
    `);
  });

  test('getBrightnessPresets', async () => {
    const actual = await PreferenceUtils.getBrightnessPresets();
    expect(actual).toMatchInlineSnapshot(`
      Array [
        Object {
          "level": 0,
        },
        Object {
          "level": 50,
        },
        Object {
          "level": 100,
        },
      ]
    `);
  });

  test('patch', async () => {
    const oldPrefs = await PreferenceUtils.get();
    const actual = await PreferenceUtils.patch({
      ...oldPrefs,
      brightnessDelta: 30,
    });
    expect(actual).toMatchInlineSnapshot(`
      Object {
        "brightnessBinary": "/usr/local/bin/brightness",
        "brightnessDelta": 30,
        "brightnessPresets": Array [
          Object {
            "level": 0,
          },
          Object {
            "level": 50,
          },
          Object {
            "level": 100,
          },
        ],
        "ddcctlBinary": "/usr/local/bin/ddcctl",
        "keyBindings": Array [
          Object {
            "command": Array [
              "command/changeDarkMode/toggle",
            ],
            "key": "Shift+Escape",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/dark",
              "command/changeBrightness/10",
            ],
            "key": "Shift+F1",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/light",
              "command/changeBrightness/100",
            ],
            "key": "Shift+F2",
          },
          Object {
            "command": Array [
              "command/changeBrightness/0",
            ],
            "key": "Shift+F3",
          },
          Object {
            "command": Array [
              "command/changeBrightness/50",
            ],
            "key": "Shift+F4",
          },
          Object {
            "command": Array [
              "command/changeBrightness/100",
            ],
            "key": "Shift+F5",
          },
        ],
        "showIndividualDisplays": false,
      }
    `);
  });
});
