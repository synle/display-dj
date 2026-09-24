import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';

const PACKAGE_JSON = fileURLToPath(new URL('../package.json', import.meta.url));
const TAURI_CONFIG = fileURLToPath(new URL('../src-tauri/tauri.conf.json', import.meta.url));
const VERSION_PROPERTY = /^(\s*"version"\s*:\s*)"[^"]*"(\s*,\s*)$/gm;
const STRICT_SEMVER = /^\d+\.\d+\.\d+(?:[-+][A-Za-z0-9.]+)?$/;

/**
 * Mirror package.json's version into tauri.conf.json without reformatting it.
 *
 * @param {{ packageJsonPath?: string, tauriConfigPath?: string }} paths
 * @returns {boolean} Whether tauri.conf.json changed.
 * @throws {Error} When package version or Tauri config shape is invalid.
 */
export function syncTauriVersion({
  packageJsonPath = PACKAGE_JSON,
  tauriConfigPath = TAURI_CONFIG,
} = {}) {
  const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf8'));
  const version = packageJson.version;
  if (typeof version !== 'string' || !STRICT_SEMVER.test(version)) {
    throw new Error(`${packageJsonPath} must contain a strict semver version`);
  }

  const tauriConfigText = readFileSync(tauriConfigPath, 'utf8');
  const tauriConfig = JSON.parse(tauriConfigText);
  if (tauriConfig.version === version) {
    return false;
  }

  if ([...tauriConfigText.matchAll(VERSION_PROPERTY)].length !== 1) {
    throw new Error(`${tauriConfigPath} must contain exactly one version property`);
  }

  const nextConfig = tauriConfigText.replace(VERSION_PROPERTY, `$1${JSON.stringify(version)}$2`);
  writeFileSync(tauriConfigPath, nextConfig);
  console.log(`Synced Tauri version to ${version}`);
  return true;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  syncTauriVersion();
}
