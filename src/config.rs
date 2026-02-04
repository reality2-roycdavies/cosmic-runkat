//! Configuration management for cosmic-runkat
//!
//! Stores user preferences including:
//! - Minimum CPU threshold for sleep mode
//! - Animation speed settings

use crate::constants::*;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Popup position relative to screen edges
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PopupPosition {
    /// Top-left corner (near left panel tray)
    TopLeft,
    /// Top-right corner (near right panel tray)
    #[default]
    TopRight,
    /// Bottom-left corner
    BottomLeft,
    /// Bottom-right corner
    BottomRight,
}

impl PopupPosition {
    /// All available positions
    pub const ALL: &'static [PopupPosition] = &[
        PopupPosition::TopLeft,
        PopupPosition::TopRight,
        PopupPosition::BottomLeft,
        PopupPosition::BottomRight,
    ];

    /// Display names matching ALL order
    pub const NAMES: &'static [&'static str] = &[
        "Top Left",
        "Top Right",
        "Bottom Left",
        "Bottom Right",
    ];
}

/// What drives the cat animation speed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationSource {
    /// Animation speed based on CPU usage (default)
    #[default]
    CpuUsage,
    /// Animation speed based on CPU frequency
    Frequency,
    /// Animation speed based on CPU temperature
    Temperature,
}

impl AnimationSource {
    /// All available sources
    pub const ALL: &'static [AnimationSource] = &[
        AnimationSource::CpuUsage,
        AnimationSource::Frequency,
        AnimationSource::Temperature,
    ];

    /// Display names matching ALL order
    pub const NAMES: &'static [&'static str] = &[
        "CPU Usage",
        "CPU Frequency",
        "CPU Temperature",
    ];
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Legacy sleep threshold (kept for backwards compatibility)
    /// Use the per-source thresholds instead
    #[serde(default = "default_sleep_threshold")]
    pub sleep_threshold: f32,

    /// Sleep threshold for CPU usage mode (percentage, 0-30)
    #[serde(default = "default_cpu_threshold")]
    pub sleep_threshold_cpu: f32,

    /// Sleep threshold for frequency mode (MHz)
    #[serde(default = "default_freq_threshold")]
    pub sleep_threshold_freq: f32,

    /// Sleep threshold for temperature mode (degrees C, 20-100)
    #[serde(default = "default_temp_threshold")]
    pub sleep_threshold_temp: f32,

    /// Maximum animation speed in frames per second (default: 15)
    pub max_fps: f32,

    /// Minimum animation speed in frames per second (default: 2)
    pub min_fps: f32,

    /// Show CPU percentage on the tray icon (default: true)
    pub show_percentage: bool,

    /// Where the popup appears when clicking the tray icon (default: top-right)
    #[serde(default)]
    pub popup_position: PopupPosition,

    /// What drives the cat animation speed and popup display (default: CPU usage)
    #[serde(default)]
    pub animation_source: AnimationSource,
}

fn default_sleep_threshold() -> f32 { 5.0 }
fn default_cpu_threshold() -> f32 { 5.0 }
fn default_freq_threshold() -> f32 { 1000.0 }  // 1 GHz
fn default_temp_threshold() -> f32 { 40.0 }

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_threshold: 5.0,
            sleep_threshold_cpu: 5.0,
            sleep_threshold_freq: 1000.0,  // 1 GHz
            sleep_threshold_temp: 40.0,
            max_fps: 15.0,
            min_fps: 2.0,
            show_percentage: true,
            popup_position: PopupPosition::default(),
            animation_source: AnimationSource::default(),
        }
    }
}

impl Config {
    /// Get the current sleep threshold based on animation source
    pub fn current_threshold(&self) -> f32 {
        match self.animation_source {
            AnimationSource::CpuUsage => self.sleep_threshold_cpu,
            AnimationSource::Frequency => self.sleep_threshold_freq,
            AnimationSource::Temperature => self.sleep_threshold_temp,
        }
    }

    /// Set the current sleep threshold based on animation source
    pub fn set_current_threshold(&mut self, value: f32) {
        match self.animation_source {
            AnimationSource::CpuUsage => self.sleep_threshold_cpu = value,
            AnimationSource::Frequency => self.sleep_threshold_freq = value,
            AnimationSource::Temperature => self.sleep_threshold_temp = value,
        }
        // Also update legacy field for compatibility
        self.sleep_threshold = value;
    }
}

impl Config {
    /// Get the config file path (Flatpak-aware)
    pub fn config_path() -> PathBuf {
        paths::app_config_dir().join("config.json")
    }

    /// Get the legacy config file path (pre-1.0 location)
    ///
    /// Used for automatic migration from 0.3.x to 1.0.0
    fn legacy_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("cosmic-runkat").join("config.json"))
    }

    /// Load configuration from disk with auto-migration and validation
    ///
    /// Attempts to load config from:
    /// 1. Current location (Flatpak-aware)
    /// 2. Legacy location (pre-1.0)
    /// 3. Defaults if neither found
    ///
    /// If legacy config is found, it's automatically migrated to the new location.
    /// Invalid configs are replaced with defaults and a warning is shown.
    pub fn load() -> Self {
        let path = Self::config_path();

        // Try current location first
        if path.exists() {
            if let Some(mut config) = Self::load_from_path(&path) {
                // Validate and fix if needed
                if let Err(e) = config.validate() {
                    tracing::warn!("Invalid config: {}. Using defaults.", e);
                    eprintln!("Warning: Invalid config: {}. Using defaults.", e);
                    config = Self::default();
                }
                return config;
            }
        }

        // Try legacy location for migration
        if let Some(legacy_path) = Self::legacy_config_path() {
            if legacy_path.exists() && legacy_path != path {
                tracing::info!("Found config at legacy location, migrating to new location");
                eprintln!("Found config at legacy location, migrating to new location...");

                if let Some(mut config) = Self::load_from_path(&legacy_path) {
                    // Validate before migrating
                    if let Err(e) = config.validate() {
                        tracing::warn!("Invalid legacy config: {}. Using defaults.", e);
                        eprintln!("Warning: Invalid legacy config: {}. Using defaults.", e);
                        config = Self::default();
                    }

                    // Migrate to new location
                    if let Err(e) = config.save() {
                        tracing::error!("Failed to migrate config: {}", e);
                        eprintln!("Warning: Failed to migrate config: {}", e);
                    } else {
                        tracing::info!("Successfully migrated config");
                        eprintln!("Successfully migrated config to new location");
                        // Remove old file after successful migration
                        let _ = fs::remove_file(legacy_path);
                    }
                    return config;
                }
            }
        }

        // No config found, use defaults
        Self::default()
    }

    /// Load config from a specific path
    fn load_from_path(path: &Path) -> Option<Self> {
        match fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => {
                    tracing::debug!("Loaded config from {:?}", path);
                    Some(config)
                }
                Err(e) => {
                    tracing::error!("Failed to parse config file: {}", e);
                    eprintln!("Failed to parse config file: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::error!("Failed to read config file: {}", e);
                eprintln!("Failed to read config file: {}", e);
                None
            }
        }
    }

    /// Save configuration to disk with validation
    pub fn save(&self) -> Result<(), std::io::Error> {
        // Validate before saving
        if let Err(e) = self.validate() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, e));
        }

        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)
    }

    /// Validate configuration values
    ///
    /// Returns an error if any values are outside acceptable ranges.
    pub fn validate(&self) -> Result<(), String> {
        // Validate per-source thresholds
        if !(0.0..=100.0).contains(&self.sleep_threshold_cpu) {
            return Err(format!(
                "sleep_threshold_cpu must be between 0 and 100, got {}",
                self.sleep_threshold_cpu
            ));
        }

        // Frequency threshold can be 0 to 10000 MHz (reasonable max)
        if !(0.0..=10000.0).contains(&self.sleep_threshold_freq) {
            return Err(format!(
                "sleep_threshold_freq must be between 0 and 10000 MHz, got {}",
                self.sleep_threshold_freq
            ));
        }

        // Temperature threshold 0 to 150°C
        if !(0.0..=150.0).contains(&self.sleep_threshold_temp) {
            return Err(format!(
                "sleep_threshold_temp must be between 0 and 150°C, got {}",
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

    /// Calculate animation FPS based on CPU usage percentage
    /// Cat always moves at least at min_fps, speeds up with CPU usage
    pub fn calculate_fps(&self, cpu_percent: f32) -> f32 {
        // Linear interpolation between min_fps and max_fps
        // based on CPU percentage (0 to 100%)
        let normalized = (cpu_percent / 100.0).clamp(0.0, 1.0);
        self.min_fps + normalized * (self.max_fps - self.min_fps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_sleep_threshold_too_low() {
        let mut config = Config::default();
        config.sleep_threshold = -5.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_sleep_threshold_too_high() {
        let mut config = Config::default();
        config.sleep_threshold = 25.0;
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
    fn test_sleep_threshold() {
        let config = Config::default();

        // Below threshold = sleeping (cpu < sleep_threshold)
        assert!(3.0 < config.sleep_threshold);
        assert!(4.9 < config.sleep_threshold);

        // At or above threshold = not sleeping
        assert!(!(5.0 < config.sleep_threshold));
        assert!(!(50.0 < config.sleep_threshold));
    }
}
