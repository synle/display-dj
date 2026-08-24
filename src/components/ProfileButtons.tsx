import { useState, useRef, useEffect } from 'react';
import { Profile } from '../types';

const MAX_VISIBLE = 3;

interface ProfileButtonsProps {
  profiles: Profile[];
  onActivate: (index: number) => void;
}

/** Returns the profile's display name, falling back to "Unnamed Profile #N". */
function profileName(profile: Profile, index: number): string {
  return profile.name || `Unnamed Profile #${index + 1}`;
}

/** Row of profile quick-action buttons with overflow menu for 4+ profiles. */
export default function ProfileButtons({ profiles, onActivate }: ProfileButtonsProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [menuOpen]);

  if (profiles.length === 0) return null;

  const visible = profiles.slice(0, MAX_VISIBLE);
  const overflow = profiles.slice(MAX_VISIBLE);

  return (
    <div className='profile-buttons'>
      {visible.map((profile, i) => (
        <button
          key={profile.name || `unnamed-${i}`}
          className='profile-btn'
          onClick={() => onActivate(i)}
          title={profileName(profile, i)}>
          {profileName(profile, i)}
        </button>
      ))}
      {overflow.length > 0 && (
        <div className='profile-overflow' ref={menuRef}>
          <button
            className='profile-btn profile-overflow-btn'
            onClick={() => setMenuOpen(!menuOpen)}
            title='More profiles'>
            {'\u25BE'}
          </button>
          {menuOpen && (
            <div className='profile-overflow-menu'>
              {overflow.map((profile, i) => {
                const actualIndex = MAX_VISIBLE + i;
                return (
                  <button
                    key={actualIndex}
                    className='profile-overflow-item'
                    onClick={() => {
                      onActivate(actualIndex);
                      setMenuOpen(false);
                    }}>
                    {profileName(profile, actualIndex)}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
