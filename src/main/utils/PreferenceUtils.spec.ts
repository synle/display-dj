import PreferenceUtils from 'src/main/utils/PreferenceUtils';
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

    delete actual.mode;

    expect(actual).toMatchInlineSnapshot(`
      Object {
        "brightnessDelta": 25,
        "brightnessPresets": Array [
          Object {
            "level": 10,
            "syncedWithMode": "dark",
          },
          Object {
            "level": 50,
          },
          Object {
            "level": 100,
            "syncedWithMode": "light",
          },
        ],
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
            "notification": "Switching to Dark Profile",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/light",
              "command/changeBrightness/100",
            ],
            "key": "Shift+F2",
            "notification": "Switching to Light Profile",
          },
          Object {
            "command": Array [
              "command/changeBrightness/10",
            ],
            "key": "Shift+F3",
            "notification": "Brightness is 10%",
          },
          Object {
            "command": Array [
              "command/changeBrightness/50",
            ],
            "key": "Shift+F4",
            "notification": "Brightness is 50%",
          },
          Object {
            "command": Array [
              "command/changeBrightness/100",
            ],
            "key": "Shift+F5",
            "notification": "Brightness is 100%",
          },
          Object {
            "command": Array [
              "command/changeVolume/0",
            ],
            "key": "Shift+F6",
            "notification": "Volume is Muted",
          },
          Object {
            "command": Array [
              "command/changeVolume/100",
            ],
            "key": "Shift+F7",
            "notification": "Volume is 100%",
          },
        ],
        "logging": false,
        "showIndividualDisplays": false,
        "volumePresets": Array [
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
          "notification": "Switching to Dark Profile",
        },
        Object {
          "command": Array [
            "command/changeDarkMode/light",
            "command/changeBrightness/100",
          ],
          "key": "Shift+F2",
          "notification": "Switching to Light Profile",
        },
        Object {
          "command": Array [
            "command/changeBrightness/10",
          ],
          "key": "Shift+F3",
          "notification": "Brightness is 10%",
        },
        Object {
          "command": Array [
            "command/changeBrightness/50",
          ],
          "key": "Shift+F4",
          "notification": "Brightness is 50%",
        },
        Object {
          "command": Array [
            "command/changeBrightness/100",
          ],
          "key": "Shift+F5",
          "notification": "Brightness is 100%",
        },
        Object {
          "command": Array [
            "command/changeVolume/0",
          ],
          "key": "Shift+F6",
          "notification": "Volume is Muted",
        },
        Object {
          "command": Array [
            "command/changeVolume/100",
          ],
          "key": "Shift+F7",
          "notification": "Volume is 100%",
        },
      ]
    `);
  });

  test('getBrightnessPresets', async () => {
    const actual = await PreferenceUtils.getBrightnessPresets();
    expect(actual).toMatchInlineSnapshot(`
      Array [
        Object {
          "level": 10,
          "syncedWithMode": "dark",
        },
        Object {
          "level": 50,
        },
        Object {
          "level": 100,
          "syncedWithMode": "light",
        },
      ]
    `);
  });

  test('getVolumePresets', async () => {
    const actual = await PreferenceUtils.getVolumePresets();
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

    delete actual.mode;

    expect(actual).toMatchInlineSnapshot(`
      Object {
        "brightnessDelta": 30,
        "brightnessPresets": Array [
          Object {
            "level": 10,
            "syncedWithMode": "dark",
          },
          Object {
            "level": 50,
          },
          Object {
            "level": 100,
            "syncedWithMode": "light",
          },
        ],
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
            "notification": "Switching to Dark Profile",
          },
          Object {
            "command": Array [
              "command/changeDarkMode/light",
              "command/changeBrightness/100",
            ],
            "key": "Shift+F2",
            "notification": "Switching to Light Profile",
          },
          Object {
            "command": Array [
              "command/changeBrightness/10",
            ],
            "key": "Shift+F3",
            "notification": "Brightness is 10%",
          },
          Object {
            "command": Array [
              "command/changeBrightness/50",
            ],
            "key": "Shift+F4",
            "notification": "Brightness is 50%",
          },
          Object {
            "command": Array [
              "command/changeBrightness/100",
            ],
            "key": "Shift+F5",
            "notification": "Brightness is 100%",
          },
          Object {
            "command": Array [
              "command/changeVolume/0",
            ],
            "key": "Shift+F6",
            "notification": "Volume is Muted",
          },
          Object {
            "command": Array [
              "command/changeVolume/100",
            ],
            "key": "Shift+F7",
            "notification": "Volume is 100%",
          },
        ],
        "logging": false,
        "showIndividualDisplays": false,
        "volumePresets": Array [
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
      }
    `);
  });
});
