//! Configuration management for cosmic-runkat
//!
//! Handles loading, saving, and validating user preferences.  The config
//! is stored as JSON on disk and automatically reloaded every 500ms by the
//! applet so that changes from the settings window take effect immediately.
//!
//! ## Config file location
//!
//! `~/.config/cosmic-runkat/config.json` (standard XDG config directory)

use crate::constants::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// What system metric drives the cat animation speed.
///
/// This enum is stored in the config file as a kebab-case string
/// (e.g. `"cpu-usage"`, `"frequency"`, `"temperature"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationSource {
    /// Cat speed reflects overall CPU usage percentage (default)
    #[default]
    CpuUsage,
    /// Cat speed reflects CPU clock frequency
    Frequency,
    /// Cat speed reflects CPU temperature
    Temperature,
}

impl AnimationSource {
    /// All variants in display order — used for dropdown menus
    pub const ALL: &'static [AnimationSource] = &[
        AnimationSource::CpuUsage,
        AnimationSource::Frequency,
        AnimationSource::Temperature,
    ];

    /// Human-readable names corresponding to `ALL` — shown in the settings UI
    pub const NAMES: &'static [&'static str] = &[
        "CPU Usage",
        "CPU Frequency",
        "CPU Temperature",
    ];
}

/// User-configurable application settings.
///
/// Serialized to/from JSON on disk.  The `#[serde(default = "...")]`
/// attributes ensure that missing fields (e.g. when upgrading from an older
/// config version) get sensible defaults rather than causing parse errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Below this CPU usage %, the cat sleeps (CPU mode only).
    /// Range: 0-30 in the settings slider, but validated 0-100.
    #[serde(default = "default_cpu_threshold")]
    pub sleep_threshold_cpu: f32,

    /// Below this frequency in MHz, the cat sleeps (frequency mode only)
    #[serde(default = "default_freq_threshold")]
    pub sleep_threshold_freq: f32,

    /// Below this temperature in C, the cat sleeps (temperature mode only)
    #[serde(default = "default_temp_threshold")]
    pub sleep_threshold_temp: f32,

    /// Fastest the animation can run (frames per second)
    pub max_fps: f32,

    /// Slowest the animation can run (frames per second)
    pub min_fps: f32,

    /// Whether to show "42%" next to the cat in CPU usage mode
    pub show_percentage: bool,

    /// Which system metric drives the animation and popup display
    #[serde(default)]
    pub animation_source: AnimationSource,
}

// Default value functions for serde — called when a field is missing from
// the JSON file.
fn default_cpu_threshold() -> f32 { 5.0 }
fn default_freq_threshold() -> f32 { 1000.0 }  // 1 GHz
fn default_temp_threshold() -> f32 { 40.0 }     // 40 C

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_threshold_cpu: 5.0,
            sleep_threshold_freq: 1000.0,
            sleep_threshold_temp: 40.0,
            max_fps: 15.0,
            min_fps: 2.0,
            show_percentage: true,
            animation_source: AnimationSource::default(),
        }
    }
}

impl Config {
    /// Get the sleep threshold for whichever animation source is currently
    /// selected.  For example, if the user is in CPU mode, returns the
    /// CPU threshold; in frequency mode, the frequency threshold, etc.
    pub fn current_threshold(&self) -> f32 {
        match self.animation_source {
            AnimationSource::CpuUsage => self.sleep_threshold_cpu,
            AnimationSource::Frequency => self.sleep_threshold_freq,
            AnimationSource::Temperature => self.sleep_threshold_temp,
        }
    }

    /// Set the sleep threshold for the currently selected animation source.
    /// Called from the settings window when the user moves the slider.
    pub fn set_current_threshold(&mut self, value: f32) {
        match self.animation_source {
            AnimationSource::CpuUsage => self.sleep_threshold_cpu = value,
            AnimationSource::Frequency => self.sleep_threshold_freq = value,
            AnimationSource::Temperature => self.sleep_threshold_temp = value,
        }
    }

    /// Full path to the config JSON file.
    ///
    /// Uses the standard XDG config directory (`~/.config/cosmic-runkat/config.json`).
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("cosmic-runkat"))
            .unwrap_or_else(|| PathBuf::from("/tmp/cosmic-runkat"))
            .join("config.json")
    }

    /// Load configuration from disk.
    ///
    /// If the config file exists and is valid, returns it.
    /// If it's missing or invalid, returns `Config::default()`.
    pub fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            if let Some(mut config) = Self::load_from_path(&path) {
                if let Err(e) = config.validate() {
                    tracing::warn!("Invalid config: {}. Using defaults.", e);
                    config = Self::default();
                }
                return config;
            }
        }

        Self::default()
    }

    /// Try to read and parse a config file at the given path.
    /// Returns `None` and logs errors if the file can't be read or parsed.
    fn load_from_path(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| tracing::error!("Failed to read config file: {}", e))
            .ok()?;

        serde_json::from_str(&content)
            .map_err(|e| tracing::error!("Failed to parse config file: {}", e))
            .ok()
            .inspect(|_| tracing::debug!("Loaded config from {:?}", path))
    }

    /// Write the config to disk as pretty-printed JSON.
    /// Validates before saving to avoid writing corrupt data.
    pub fn save(&self) -> Result<(), std::io::Error> {
        self.validate()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

        let path = Self::config_path();
        // Ensure the parent directory exists (e.g. on first run)
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)
    }

    /// Check that all config values are within acceptable ranges.
    /// Returns `Err(message)` describing the first invalid value found.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=100.0).contains(&self.sleep_threshold_cpu) {
            return Err(format!(
                "sleep_threshold_cpu must be between 0 and 100, got {}",
                self.sleep_threshold_cpu
            ));
        }

        if !(0.0..=10000.0).contains(&self.sleep_threshold_freq) {
            return Err(format!(
                "sleep_threshold_freq must be between 0 and 10000 MHz, got {}",
                self.sleep_threshold_freq
            ));
        }

        if !(0.0..=150.0).contains(&self.sleep_threshold_temp) {
            return Err(format!(
                "sleep_threshold_temp must be between 0 and 150\u{00b0}C, got {}",
                self.sleep_threshold_temp
            ));
        }

        if !(MIN_FPS..=MAX_FPS).contains(&self.min_fps) {
            return Err(format!(
                "min_fps must be between {} and {}, got {}",
                MIN_FPS, MAX_FPS, self.min_fps
            ));
        }

        if !(MIN_FPS..=MAX_FPS).contains(&self.max_fps) {
            return Err(format!(
                "max_fps must be between {} and {}, got {}",
                MIN_FPS, MAX_FPS, self.max_fps
            ));
        }

        if self.min_fps >= self.max_fps {
            return Err(format!(
                "min_fps ({}) must be less than max_fps ({})",
                self.min_fps, self.max_fps
            ));
        }

        Ok(())
    }

    /// Calculate how fast the cat should animate based on a 0-100% metric.
    ///
    /// Uses linear interpolation: at 0% -> `min_fps`, at 100% -> `max_fps`.
    /// Values outside 0-100 are clamped.
    pub fn calculate_fps(&self, cpu_percent: f32) -> f32 {
        let normalized = (cpu_percent / 100.0).clamp(0.0, 1.0);
        self.min_fps + normalized * (self.max_fps - self.min_fps)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_cpu_threshold_too_low() {
        let mut config = Config::default();
        config.sleep_threshold_cpu = -5.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_cpu_threshold_too_high() {
        let mut config = Config::default();
        config.sleep_threshold_cpu = 150.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_min_fps_invalid() {
        let mut config = Config::default();
        config.min_fps = 0.5;
        assert!(config.validate().is_err());

        config = Config::default();
        config.min_fps = 35.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_fps_order_invalid() {
        let mut config = Config::default();
        config.min_fps = 20.0;
        config.max_fps = 10.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_fps_calculation() {
        let config = Config::default();

        // At very low CPU, fps should be close to min_fps
        let low_fps = config.calculate_fps(3.0);
        assert!(low_fps >= config.min_fps);
        assert!(low_fps < config.min_fps + 1.0);

        // At 100% CPU = max fps
        assert!((config.calculate_fps(100.0) - config.max_fps).abs() < 0.01);
    }

    #[test]
    fn test_current_threshold() {
        let mut config = Config::default();

        config.animation_source = AnimationSource::CpuUsage;
        assert!((config.current_threshold() - 5.0).abs() < f32::EPSILON);

        config.animation_source = AnimationSource::Frequency;
        assert!((config.current_threshold() - 1000.0).abs() < f32::EPSILON);

        config.animation_source = AnimationSource::Temperature;
        assert!((config.current_threshold() - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_set_current_threshold() {
        let mut config = Config::default();

        config.animation_source = AnimationSource::CpuUsage;
        config.set_current_threshold(10.0);
        assert!((config.sleep_threshold_cpu - 10.0).abs() < f32::EPSILON);

        config.animation_source = AnimationSource::Frequency;
        config.set_current_threshold(2000.0);
        assert!((config.sleep_threshold_freq - 2000.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_path_ends_with_expected() {
        let path = Config::config_path();
        assert!(path.ends_with("cosmic-runkat/config.json"));
    }
}
