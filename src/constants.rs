//! Application-wide constants
//!
//! Centralizes all magic numbers and configuration values for easy tuning.

use std::time::Duration;

// === Animation Constants ===

/// Number of animation frames in the run cycle
pub const RUN_FRAMES: u8 = 10;

/// Cat sprite dimensions (square)
pub const CAT_SIZE: u32 = 32;

// === CPU Monitoring Constants ===

/// Number of CPU samples to average for smoothing.
///
/// At 500ms sample interval, this is 5 seconds of history.
pub const CPU_SAMPLE_COUNT: usize = 10;

/// CPU sample interval for systemstat
pub const CPU_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);

// === Config Validation Constants ===

/// Minimum allowed animation FPS
pub const MIN_FPS: f32 = 1.0;

/// Maximum allowed animation FPS
pub const MAX_FPS: f32 = 30.0;

// === Popup Sizing Constants ===

/// Popup width in pixels
pub const POPUP_WIDTH: u32 = 340;

/// Base popup height (title + dividers + status + padding)
pub const POPUP_BASE_HEIGHT: u32 = 100;

/// Height per data row in the popup
pub const POPUP_ROW_HEIGHT: u32 = 20;

/// Maximum visible rows before scrolling (24 cores + 1 summary)
pub const POPUP_MAX_ROWS: u32 = 25;

/// Maximum scrollable content height in the popup
pub const POPUP_MAX_SCROLL_HEIGHT: f32 = 500.0;

// === Progress Bar Constants ===

/// Width of progress bars in the popup
pub const BAR_WIDTH: f32 = 140.0;

/// Height of progress bars in the popup
pub const BAR_HEIGHT: f32 = 12.0;

/// Temperature threshold (C) above which the "HOT" status is shown
pub const TEMP_HOT_THRESHOLD: f32 = 80.0;
