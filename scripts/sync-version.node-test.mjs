import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { syncTauriVersion } from './sync-version.mjs';

/** Syncs a stale Tauri version while preserving unrelated formatting. */
test('syncs package version into Tauri config', () => {
  const directory = mkdtempSync(join(tmpdir(), 'display-dj-version-'));
  const packageJsonPath = join(directory, 'package.json');
  const tauriConfigPath = join(directory, 'tauri.conf.json');
  writeFileSync(packageJsonPath, '{"version":"9.14.0"}\n');
  writeFileSync(
    tauriConfigPath,
    '{\n  "productName": "Display DJ",\n  "version": "9.13.0",\n  "bundle": {"active": true}\n}\n',
  );

  try {
    assert.equal(syncTauriVersion({ packageJsonPath, tauriConfigPath }), true);
    assert.equal(
      readFileSync(tauriConfigPath, 'utf8'),
      '{\n  "productName": "Display DJ",\n  "version": "9.14.0",\n  "bundle": {"active": true}\n}\n',
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

/** Rejects invalid package versions without modifying Tauri config. */
test('rejects a package version that is not strict semver', () => {
  const directory = mkdtempSync(join(tmpdir(), 'display-dj-version-'));
  const packageJsonPath = join(directory, 'package.json');
  const tauriConfigPath = join(directory, 'tauri.conf.json');
  const tauriConfig = '{\n  "version": "9.13.0"\n}\n';
  writeFileSync(packageJsonPath, '{"version":"next"}\n');
  writeFileSync(tauriConfigPath, tauriConfig);

  try {
    assert.throws(
      () => syncTauriVersion({ packageJsonPath, tauriConfigPath }),
      /must contain a strict semver version/,
    );
    assert.equal(readFileSync(tauriConfigPath, 'utf8'), tauriConfig);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
