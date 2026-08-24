//! TTL-based cache for platform reads.
//!
//! Caches the results of `core::PlatformImpl::enumerate()` (monitors),
//! `core::theme::get_dark_mode()`, and `core::volume::get_volume()` so that
//! rapid frontend polls don't re-hit the OS APIs on every call. The TTL is
//! 5 minutes — stale entries trigger a fresh in-process read. Writes
//! (set_brightness, set_volume, set_dark_mode, etc.) and Force Refresh
//! invalidate the relevant cache entries.
//!
//! Name retained from v6.x (when the cache wrapped HTTP responses from the
//! sidecar process). v7+ has no sidecar — this is now a pure in-process
//! cache. See AGENTS.md "Architecture".

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cache TTL: entries older than this are considered stale.
const CACHE_TTL: Duration = Duration::from_secs(300); // 5 minutes

/// A single cached value with a timestamp.
struct CacheEntry<T> {
    value: T,
    fetched_at: Instant,
}

impl<T> CacheEntry<T> {
    /// Returns true if the entry is still fresh (within TTL).
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

/// Thread-safe cache for platform reads and system state.
pub struct SidecarCache {
    /// Cached monitor list (from `core::PlatformImpl::enumerate()` + metadata merge).
    monitors: Mutex<Option<CacheEntry<Vec<crate::display::Monitor>>>>,
    /// Cached dark mode state (from `core::theme::get_dark_mode()`).
    dark_mode: Mutex<Option<CacheEntry<bool>>>,
    /// Cached volume level (from `core::volume::get_volume()`).
    volume: Mutex<Option<CacheEntry<u32>>>,
    /// Cached macOS Accessibility permission status (AXIsProcessTrusted).
    accessibility: Mutex<Option<CacheEntry<bool>>>,
}

impl SidecarCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            monitors: Mutex::new(None),
            dark_mode: Mutex::new(None),
            volume: Mutex::new(None),
            accessibility: Mutex::new(None),
        }
    }

    /// Get cached monitors if fresh, otherwise None.
    pub fn get_monitors(&self) -> Option<Vec<crate::display::Monitor>> {
        let lock = self.monitors.lock().ok()?;
        lock.as_ref()
            .filter(|e| e.is_fresh())
            .map(|e| e.value.clone())
    }

    /// Store monitors in cache.
    pub fn set_monitors(&self, monitors: Vec<crate::display::Monitor>) {
        if let Ok(mut lock) = self.monitors.lock() {
            *lock = Some(CacheEntry {
                value: monitors,
                fetched_at: Instant::now(),
            });
        }
    }

    /// Get cached dark mode if fresh, otherwise None.
    pub fn get_dark_mode(&self) -> Option<bool> {
        let lock = self.dark_mode.lock().ok()?;
        lock.as_ref().filter(|e| e.is_fresh()).map(|e| e.value)
    }

    /// Store dark mode in cache.
    pub fn set_dark_mode(&self, is_dark: bool) {
        if let Ok(mut lock) = self.dark_mode.lock() {
            *lock = Some(CacheEntry {
                value: is_dark,
                fetched_at: Instant::now(),
            });
        }
    }

    /// Get cached volume if fresh, otherwise None.
    pub fn get_volume(&self) -> Option<u32> {
        let lock = self.volume.lock().ok()?;
        lock.as_ref().filter(|e| e.is_fresh()).map(|e| e.value)
    }

    /// Store volume in cache.
    pub fn set_volume(&self, volume: u32) {
        if let Ok(mut lock) = self.volume.lock() {
            *lock = Some(CacheEntry {
                value: volume,
                fetched_at: Instant::now(),
            });
        }
    }

    /// Get cached accessibility status if fresh, otherwise None.
    pub fn get_accessibility(&self) -> Option<bool> {
        let lock = self.accessibility.lock().ok()?;
        lock.as_ref().filter(|e| e.is_fresh()).map(|e| e.value)
    }

    /// Store accessibility status in cache.
    pub fn set_accessibility(&self, trusted: bool) {
        if let Ok(mut lock) = self.accessibility.lock() {
            *lock = Some(CacheEntry {
                value: trusted,
                fetched_at: Instant::now(),
            });
        }
    }

    /// Invalidate the accessibility cache.
    pub fn invalidate_accessibility(&self) {
        if let Ok(mut lock) = self.accessibility.lock() {
            *lock = None;
        }
    }

    /// Invalidate the monitors cache (e.g., after set_brightness).
    pub fn invalidate_monitors(&self) {
        if let Ok(mut lock) = self.monitors.lock() {
            *lock = None;
        }
    }

    /// Invalidate the dark mode cache (e.g., after set_dark_mode).
    pub fn invalidate_dark_mode(&self) {
        if let Ok(mut lock) = self.dark_mode.lock() {
            *lock = None;
        }
    }

    /// Invalidate the volume cache (e.g., after set_volume).
    pub fn invalidate_volume(&self) {
        if let Ok(mut lock) = self.volume.lock() {
            *lock = None;
        }
    }

    /// Invalidate all cached values (e.g., Force Refresh, Reset to Default).
    pub fn invalidate_all(&self) {
        self.invalidate_monitors();
        self.invalidate_dark_mode();
        self.invalidate_volume();
        self.invalidate_accessibility();
    }
}

impl Default for SidecarCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor(id: &str) -> crate::display::Monitor {
        crate::display::Monitor {
            id: id.into(),
            uid: format!("{}::Test", id),
            name: "Test".into(),
            original_name: "Test".into(),
            brightness: 50,
            contrast: None,
            supports_brightness: true,
            is_built_in: false,
            hidden: false,
            monitor_rect: None,
        }
    }

    #[test]
    fn test_new_cache_is_empty() {
        let cache = SidecarCache::new();
        assert!(cache.get_monitors().is_none());
        assert!(cache.get_dark_mode().is_none());
        assert!(cache.get_volume().is_none());
        assert!(cache.get_accessibility().is_none());
    }

    #[test]
    fn test_default_equivalent_to_new() {
        let cache = SidecarCache::default();
        assert!(cache.get_monitors().is_none());
        assert!(cache.get_dark_mode().is_none());
        assert!(cache.get_volume().is_none());
        assert!(cache.get_accessibility().is_none());
    }

    #[test]
    fn test_set_get_monitors() {
        let cache = SidecarCache::new();
        cache.set_monitors(vec![make_monitor("1"), make_monitor("2")]);
        let got = cache.get_monitors().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "1");
        assert_eq!(got[1].id, "2");
    }

    #[test]
    fn test_set_get_dark_mode() {
        let cache = SidecarCache::new();
        cache.set_dark_mode(true);
        assert_eq!(cache.get_dark_mode(), Some(true));
        cache.set_dark_mode(false);
        assert_eq!(cache.get_dark_mode(), Some(false));
    }

    #[test]
    fn test_set_get_volume() {
        let cache = SidecarCache::new();
        cache.set_volume(75);
        assert_eq!(cache.get_volume(), Some(75));
        cache.set_volume(0);
        assert_eq!(cache.get_volume(), Some(0));
    }

    #[test]
    fn test_set_get_accessibility() {
        let cache = SidecarCache::new();
        cache.set_accessibility(true);
        assert_eq!(cache.get_accessibility(), Some(true));
        cache.set_accessibility(false);
        assert_eq!(cache.get_accessibility(), Some(false));
    }

    #[test]
    fn test_invalidate_clears_individual_caches() {
        let cache = SidecarCache::new();
        cache.set_monitors(vec![make_monitor("1")]);
        cache.set_dark_mode(true);
        cache.set_volume(50);
        cache.set_accessibility(true);

        cache.invalidate_monitors();
        assert!(cache.get_monitors().is_none());
        assert!(cache.get_dark_mode().is_some());

        cache.invalidate_dark_mode();
        assert!(cache.get_dark_mode().is_none());
        assert!(cache.get_volume().is_some());

        cache.invalidate_volume();
        assert!(cache.get_volume().is_none());
        assert!(cache.get_accessibility().is_some());

        cache.invalidate_accessibility();
        assert!(cache.get_accessibility().is_none());
    }

    #[test]
    fn test_invalidate_all_clears_everything() {
        let cache = SidecarCache::new();
        cache.set_monitors(vec![make_monitor("1")]);
        cache.set_dark_mode(true);
        cache.set_volume(50);
        cache.set_accessibility(true);

        cache.invalidate_all();

        assert!(cache.get_monitors().is_none());
        assert!(cache.get_dark_mode().is_none());
        assert!(cache.get_volume().is_none());
        assert!(cache.get_accessibility().is_none());
    }

    #[test]
    fn test_overwrite_resets_freshness() {
        let cache = SidecarCache::new();
        cache.set_volume(10);
        cache.set_volume(20);
        assert_eq!(cache.get_volume(), Some(20));
    }
}
