//! Application-wide constants
//!
//! Centralizes all magic numbers and configuration values for easy tuning.

#![allow(dead_code)]

use std::time::Duration;

// === Animation Constants ===

/// Number of animation frames in the run cycle
pub const RUN_FRAMES: u8 = 10;

/// Cat sprite dimensions (square)
pub const CAT_SIZE: u32 = 32;

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

/// Minimum allowed animation FPS
pub const MIN_FPS: f32 = 1.0;

/// Maximum allowed animation FPS
pub const MAX_FPS: f32 = 30.0;
