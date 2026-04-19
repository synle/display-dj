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

/** About panel showing app version, update status, engine, build info,
 * homepage link, and macOS troubleshooting commands. */
function AboutPanel({ onClose }: AboutPanelProps) {
  const [info, setInfo] = useState<Record<string, string>>({});
  const [latestVersion, setLatestVersion] = useState('checking...');
  const [updateStatus, setUpdateStatus] = useState<'checking' | 'up-to-date' | 'update-available'>(
    'checking',
  );

  useEffect(() => {
    invoke<Record<string, string>>('get_about_info').then(setInfo).catch(console.error);

    fetch('https://api.github.com/repos/synle/display-dj/releases/latest')
      .then((r) => r.json())
      .then((r) => {
        const tag = r.tag_name || 'unknown';
        setLatestVersion(tag);
        return tag;
      })
      .then((tag) => {
        invoke<string>('get_app_version').then((current) => {
          const currentClean = current.split(' ')[0];
          if (tag === 'unknown') {
            setUpdateStatus('up-to-date');
          } else if (compareSemver(tag, currentClean) > 0) {
            setUpdateStatus('update-available');
          } else {
            setUpdateStatus('up-to-date');
          }
        });
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
      <div className='settings-content'>
        <div style={{ marginBottom: '8px' }}>
          {updateStatus === 'checking' && (
            <span
              style={{
                background: '#888',
                color: '#fff',
                padding: '2px 8px',
                borderRadius: '4px',
                fontSize: '11px',
              }}>
              Checking...
            </span>
          )}
          {updateStatus === 'up-to-date' && (
            <span
              style={{
                background: '#4caf50',
                color: '#fff',
                padding: '2px 8px',
                borderRadius: '4px',
                fontSize: '11px',
              }}>
              Up to date
            </span>
          )}
          {updateStatus === 'update-available' && (
            <span
              style={{
                background: '#ff9800',
                color: '#fff',
                padding: '2px 8px',
                borderRadius: '4px',
                fontSize: '11px',
              }}>
              Update available
            </span>
          )}
        </div>

        <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
          <tbody>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold', width: '90px' }}>Version</td>
              <td style={{ padding: '3px 0' }}>{info.version || '...'}</td>
            </tr>
            <tr>
              <td style={{ padding: '3px 0', fontWeight: 'bold' }}>Latest</td>
              <td style={{ padding: '3px 0' }}>{latestVersion}</td>
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
          <a
            href={info.homepage || '#'}
            target='_blank'
            rel='noopener noreferrer'
            style={{ color: '#2196f3' }}>
            synle/display-dj
          </a>
        </div>

        {updateStatus === 'update-available' && (
          <div style={{ marginTop: '6px', fontSize: '12px' }}>
            <a
              href='https://github.com/synle/display-dj/releases/latest'
              target='_blank'
              rel='noopener noreferrer'
              style={{ color: '#ff9800' }}>
              Download {latestVersion}
            </a>
          </div>
        )}

        {isMac && (
          <div style={{ marginTop: '12px', borderTop: '1px solid #333', paddingTop: '10px' }}>
            <div style={{ fontSize: '11px', marginBottom: '6px' }}>
              <strong>macOS Troubleshooting:</strong> If you see "app is damaged", run in Terminal:
            </div>
            <code
              style={{
                display: 'block',
                background: '#1a1a1a',
                color: '#ccc',
                padding: '6px 8px',
                borderRadius: '4px',
                fontSize: '11px',
                userSelect: 'all',
                marginBottom: '8px',
              }}>
              xattr -cr "/Applications/Display DJ.app"
            </code>
            <div style={{ fontSize: '11px', marginBottom: '6px' }}>
              <strong>Tiling Permission:</strong> Open Accessibility settings to grant tiling
              access:
            </div>
            <code
              style={{
                display: 'block',
                background: '#1a1a1a',
                color: '#ccc',
                padding: '6px 8px',
                borderRadius: '4px',
                fontSize: '11px',
                userSelect: 'all',
              }}>
              open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            </code>
          </div>
        )}
      </div>
    </div>
  );
}

export default AboutPanel;
