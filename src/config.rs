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

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Minimum CPU percentage below which the cat sleeps (default: 5%)
    pub sleep_threshold: f32,

    /// Maximum animation speed in frames per second (default: 15)
    pub max_fps: f32,

    /// Minimum animation speed in frames per second (default: 2)
    pub min_fps: f32,

    /// Show CPU percentage on the tray icon (default: true)
    pub show_percentage: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_threshold: 5.0,
            max_fps: 15.0, // Faster max animation
            min_fps: 2.0,  // Faster min animation
            show_percentage: true,
        }
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
        if !(MIN_SLEEP_THRESHOLD..=MAX_SLEEP_THRESHOLD).contains(&self.sleep_threshold) {
            return Err(format!(
                "sleep_threshold must be between {} and {}, got {}",
                MIN_SLEEP_THRESHOLD, MAX_SLEEP_THRESHOLD, self.sleep_threshold
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
