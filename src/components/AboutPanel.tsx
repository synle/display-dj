import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

/** Props for the AboutPanel component. */
interface AboutPanelProps {
  onClose: () => void;
}

/** Compare two semver strings. Returns 1 if a > b, -1 if a < b, 0 if equal. */
function compareSemver(a: string, b: string): number {
  const pa = a.replace(/^v/, '').split('.').map(Number);
  const pb = b.replace(/^v/, '').split('.').map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] || 0) > (pb[i] || 0)) return 1;
    if ((pa[i] || 0) < (pb[i] || 0)) return -1;
  }
  return 0;
}

/** Format a GitHub ISO timestamp (e.g. "2026-05-13T22:46:56Z") as
 * "yyyy-mm-dd HH:mm" in the user's local timezone. Returns an empty
 * string for falsy, non-string, or unparseable input so the caller can
 * conditionally hide the suffix.
 */
function formatPublishedAt(iso: string | undefined | null): string {
  if (!iso) return '';
  const d = new Date(iso);
  if (isNaN(d.getTime())) return '';
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** About panel showing app version, update status, engine, build info,
 * homepage link, and macOS troubleshooting commands. */
function AboutPanel({ onClose }: AboutPanelProps) {
  const [info, setInfo] = useState<Record<string, string>>({});
  const [latestVersion, setLatestVersion] = useState('checking...');
  const [latestDate, setLatestDate] = useState('');
  const [currentDate, setCurrentDate] = useState('');
  const [updateStatus, setUpdateStatus] = useState<'checking' | 'up-to-date' | 'update-available'>(
    'checking',
  );

  useEffect(() => {
    invoke<Record<string, string>>('get_about_info').then(setInfo).catch(console.error);

    // Fetch /releases/latest in parallel with get_app_version. We need both
    // to decide whether to issue a second fetch for the current version's
    // own release record (only when current != latest).
    Promise.all([
      invoke<string>('get_app_version'),
      fetch('https://api.github.com/repos/synle/display-dj/releases/latest').then((r) => r.json()),
    ])
      .then(async ([current, latestRelease]) => {
        const currentClean = current.split(' ')[0];
        const tag = latestRelease.tag_name || 'unknown';
        const displayTag = tag === 'unknown' ? tag : tag.replace(/^v/, '');
        setLatestVersion(displayTag);
        setLatestDate(formatPublishedAt(latestRelease.published_at));

        if (tag === 'unknown') {
          setUpdateStatus('up-to-date');
        } else if (compareSemver(tag, currentClean) > 0) {
          setUpdateStatus('update-available');
        } else {
          setUpdateStatus('up-to-date');
        }

        // If the running version is the latest, reuse the date we already
        // have. Otherwise look up the release for this exact version so the
        // "Version" row shows when *this build's* release was published.
        if (tag !== 'unknown' && displayTag === currentClean) {
          setCurrentDate(formatPublishedAt(latestRelease.published_at));
        } else if (currentClean) {
          try {
            const r = await fetch(
              `https://api.github.com/repos/synle/display-dj/releases/tags/v${currentClean}`,
            );
            const data = await r.json();
            setCurrentDate(formatPublishedAt(data?.published_at));
          } catch {
            // Local/dev builds may not have a published release — leave blank.
          }
        }
      })
      .catch(() => {
        setLatestVersion('unknown');
        setUpdateStatus('up-to-date');
      });
  }, []);

  const isMac = info.os === 'macOS';

  return (
    <div className='settings-panel'>
      <div className='settings-header'>
        <span className='settings-title'>About</span>
        <button className='settings-close' onClick={onClose} title='Close'>
          &times;
        </button>
      </div>
      <div className='settings-content about-content'>
        <div style={{ marginBottom: '8px' }}>
          {updateStatus === 'checking' && (
            <span className='about-badge about-badge-checking'>Checking...</span>
          )}
          {updateStatus === 'up-to-date' && (
            <span className='about-badge about-badge-up-to-date'>Up to date</span>
          )}
          {updateStatus === 'update-available' && (
            <span className='about-badge about-badge-update'>Update available</span>
          )}
        </div>

        <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
          <tbody>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold', width: '90px' }}>Version</td>
              <td style={{ padding: '3px 0' }}>
                {info.version || '...'}
                {currentDate && ` (${currentDate})`}
              </td>
            </tr>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold' }}>Latest</td>
              <td style={{ padding: '3px 0' }}>
                {latestVersion}
                {latestDate && ` (${latestDate})`}
              </td>
            </tr>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold' }}>Engine</td>
              <td style={{ padding: '3px 0' }}>{info.engine || '...'}</td>
            </tr>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold' }}>Platform</td>
              <td style={{ padding: '3px 0' }}>
                {info.os || '...'} ({info.arch || '...'})
              </td>
            </tr>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold' }}>Built</td>
              <td style={{ padding: '3px 0' }}>{info.buildDate || '...'}</td>
            </tr>
          </tbody>
        </table>

        <div style={{ marginTop: '10px', fontSize: '12px' }}>
          <span style={{ fontWeight: 'bold' }}>Home</span>{' '}
          <a href={info.homepage || '#'} target='_blank' rel='noopener noreferrer'>
            synle/display-dj
          </a>
        </div>

        {updateStatus === 'update-available' && (
          <div style={{ marginTop: '6px', fontSize: '12px' }}>
            <a
              href='https://github.com/synle/display-dj/releases/latest'
              target='_blank'
              rel='noopener noreferrer'
              className='about-download-link'>
              Download {latestVersion}
            </a>
          </div>
        )}

        {isMac && (
          <div className='about-macos-section'>
            <div style={{ fontSize: '11px', marginBottom: '6px' }}>
              <strong>macOS Troubleshooting:</strong> If you see "app is damaged", run in Terminal:
            </div>
            <code className='about-code-block'>xattr -cr "/Applications/Display DJ.app"</code>
            <div style={{ fontSize: '11px', marginBottom: '6px' }}>
              <strong>Tiling Permission:</strong> Open Accessibility settings to grant tiling
              access:
            </div>
            <code className='about-code-block'>
              open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            </code>
          </div>
        )}
      </div>
    </div>
  );
}

export default AboutPanel;
