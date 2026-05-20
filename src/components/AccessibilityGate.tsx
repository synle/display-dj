import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Props for the AccessibilityGate component. */
interface AccessibilityGateProps {
  /** Called when a recheck confirms the permission has been granted. Parent
   * should hide the gate and re-fetch any state that was previously blocked. */
  onGranted: () => void;
}

/** macOS-only blocking gate shown when Accessibility permission is missing.
 *
 * Tile Snap, window tiling, exposé, and z-order commands all silently no-op
 * without `AXIsProcessTrusted == true`. Rather than hand the user a popup
 * that auto-opens System Settings (which they then have to find the app
 * again to interact with), we render this as the entire popup body so the
 * problem and the fix are in the same place.
 *
 * The "I've granted it — recheck" button calls `recheck_accessibility_trusted`,
 * which bypasses the 5-minute TTL cache; if it now returns true, the parent
 * dismisses the gate and the normal UI returns. */
export default function AccessibilityGate({ onGranted }: AccessibilityGateProps) {
  const [checking, setChecking] = useState(false);
  const [lastCheckFailed, setLastCheckFailed] = useState(false);

  /** Open System Settings → Privacy & Security → Accessibility. */
  const handleOpenSettings = () => {
    invoke('open_accessibility_settings').catch(() => {});
  };

  /** Force a fresh permission check (bypasses TTL cache). On success the
   * parent unmounts the gate; on failure we surface a one-line hint. */
  const handleRecheck = async () => {
    setChecking(true);
    setLastCheckFailed(false);
    try {
      const trusted = await invoke<boolean>('recheck_accessibility_trusted');
      if (trusted) {
        onGranted();
      } else {
        setLastCheckFailed(true);
      }
    } catch {
      setLastCheckFailed(true);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className='accessibility-gate'>
      <div className='accessibility-gate-title'>Accessibility permission required</div>
      <div className='accessibility-gate-body'>
        Display DJ needs the macOS <strong>Accessibility</strong> permission to manage windows
        (tiling, Tile Snap, exposé, z-order). Without it, every tile keybinding silently no-ops.
      </div>
      <ol className='accessibility-gate-steps'>
        <li>
          Click <strong>Open Accessibility Settings</strong> below.
        </li>
        <li>
          Enable the toggle for <strong>Display DJ</strong>.
        </li>
        <li>
          Come back and click <strong>I&rsquo;ve granted it — recheck</strong>.
        </li>
      </ol>
      <div className='accessibility-gate-actions'>
        <button
          type='button'
          className='accessibility-gate-button-primary'
          onClick={handleOpenSettings}>
          Open Accessibility Settings
        </button>
        <button
          type='button'
          className='accessibility-gate-button-secondary'
          onClick={handleRecheck}
          disabled={checking}>
          {checking ? 'Checking…' : "I've granted it — recheck"}
        </button>
      </div>
      {lastCheckFailed && (
        <div className='accessibility-gate-hint'>
          Still not granted. Make sure the toggle next to <em>Display DJ</em> is on, then click
          recheck again.
        </div>
      )}
    </div>
  );
}
