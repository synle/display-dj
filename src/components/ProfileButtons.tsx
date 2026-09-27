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
  const triggerRef = useRef<HTMLButtonElement>(null);

  /** Moves focus within the overflow menu and supports Escape dismissal. */
  const handleMenuKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const items = Array.from(
      e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'),
    );
    const currentIndex = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      setMenuOpen(false);
      triggerRef.current?.focus();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      items[(currentIndex + 1) % items.length]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      items[(currentIndex - 1 + items.length) % items.length]?.focus();
    }
  };

  useEffect(() => {
    if (!menuOpen) return;
    const closeMenu = (restoreFocus: boolean) => {
      setMenuOpen(false);
      if (restoreFocus) requestAnimationFrame(() => triggerRef.current?.focus());
    };
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        closeMenu(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeMenu(true);
    };
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKeyDown);
    };
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
            ref={triggerRef}
            className='profile-btn profile-overflow-btn'
            onClick={() => {
              setMenuOpen(!menuOpen);
              if (!menuOpen)
                requestAnimationFrame(() =>
                  menuRef.current?.querySelector<HTMLButtonElement>('[role="menuitem"]')?.focus(),
                );
            }}
            aria-label='More profiles'
            aria-haspopup='menu'
            aria-expanded={menuOpen}
            title='More profiles'>
            {'\u25BE'}
          </button>
          {menuOpen && (
            <div className='profile-overflow-menu' role='menu' onKeyDown={handleMenuKeyDown}>
              {overflow.map((profile, i) => {
                const actualIndex = MAX_VISIBLE + i;
                return (
                  <button
                    key={actualIndex}
                    className='profile-overflow-item'
                    role='menuitem'
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
