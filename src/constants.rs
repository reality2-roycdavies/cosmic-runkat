//! Application-wide constants
//!
//! Centralizes all magic numbers and configuration values for easy tuning.

#![allow(dead_code)] // Some constants used in later phases

use std::time::Duration;

// === Timing Constants ===

/// Delay on startup to ensure StatusNotifierWatcher is ready
///
/// Prevents race condition when autostarting at login before
/// the panel's D-Bus services are fully initialized.
pub const STARTUP_DELAY: Duration = Duration::from_secs(2);

/// Time jump threshold for suspend/resume detection
///
/// If a single loop iteration takes longer than this, we assume
/// the system was suspended and trigger a D-Bus reconnection.
pub const SUSPEND_RESUME_THRESHOLD: Duration = Duration::from_secs(5);

/// How long before a lockfile is considered stale
///
/// Reduced from 60s to 45s to provide safety margin with refresh interval.
/// This prevents false "already running" errors when a process crashes.
pub const LOCKFILE_STALE_THRESHOLD: Duration = Duration::from_secs(45);

/// How often to refresh lockfile timestamp
///
/// Reduced from 30s to 20s to ensure 25s buffer before staleness.
/// The 25-second margin prevents race conditions where a slow system
/// might briefly cross the stale threshold during normal operation.
pub const LOCKFILE_REFRESH_INTERVAL: Duration = Duration::from_secs(20);

/// Interval for polling config/theme changes (backup to inotify)
///
/// File watchers can be unreliable, so we poll periodically as backup.
pub const CONFIG_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Delay before restarting tray after suspend/resume
pub const SUSPEND_RESTART_DELAY: Duration = Duration::from_millis(500);

/// Delay after tray shutdown to ensure D-Bus cleanup
pub const DBUS_CLEANUP_DELAY: Duration = Duration::from_millis(100);

// === Animation Constants ===

/// Number of animation frames in the run cycle
pub const RUN_FRAMES: u8 = 10;

/// Cat sprite dimensions (square)
pub const CAT_SIZE: u32 = 32;

/// Scaled cat size for small panels (XS, S)
pub const CAT_SIZE_SCALED: u32 = 48;

/// Digit sprite dimensions
pub const DIGIT_WIDTH: u32 = 8;
pub const DIGIT_HEIGHT: u32 = 12;

/// Spacing between cat and percentage text
pub const CAT_PCT_SPACING: u32 = 2;

// === CPU Monitoring Constants ===

/// Number of CPU samples to average for smoothing
///
/// At 500ms sample interval, this is 5 seconds of history.
pub const CPU_SAMPLE_COUNT: usize = 10;

/// CPU sample interval for systemstat
pub const CPU_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

/// Minimum CPU change to trigger display update (percentage points)
pub const CPU_DISPLAY_THRESHOLD: f32 = 0.5;

// === Config Validation Constants ===

/// Minimum allowed sleep threshold (percentage)
pub const MIN_SLEEP_THRESHOLD: f32 = 0.0;

/// Maximum allowed sleep threshold (percentage)
pub const MAX_SLEEP_THRESHOLD: f32 = 20.0;

/// Minimum allowed animation FPS
pub const MIN_FPS: f32 = 1.0;

/// Maximum allowed animation FPS
pub const MAX_FPS: f32 = 30.0;
