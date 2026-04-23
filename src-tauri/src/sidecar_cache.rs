//! TTL-based cache for sidecar HTTP responses.
//!
//! Caches the results of `/get_all` (monitors), `/theme` (dark mode), and
//! `/get_volume` (volume) so that rapid frontend polls don't hit the sidecar
//! on every call. The TTL is 2 minutes — stale entries trigger a fresh HTTP
//! call. Writes (set_brightness, set_volume, set_dark_mode, etc.) and Force
//! Refresh invalidate the relevant cache entries.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Cache TTL: entries older than this are considered stale.
const CACHE_TTL: Duration = Duration::from_secs(120); // 2 minutes

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

/// Thread-safe cache for sidecar HTTP responses and system state.
pub struct SidecarCache {
    /// Cached monitor list (from sidecar /get_all + metadata merge).
    monitors: Mutex<Option<CacheEntry<Vec<crate::display::Monitor>>>>,
    /// Cached dark mode state (from sidecar /theme).
    dark_mode: Mutex<Option<CacheEntry<bool>>>,
    /// Cached volume level (from sidecar /get_volume).
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
