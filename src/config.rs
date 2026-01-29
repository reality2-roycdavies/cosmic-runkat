//! Configuration management for cosmic-runkat
//!
//! Stores user preferences including:
//! - Minimum CPU threshold for sleep mode
//! - Animation speed settings

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
    /// Get the config file path
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cosmic-runkat")
            .join("config.json")
    }

    /// Load configuration from disk, or return defaults
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => config,
                    Err(e) => {
                        eprintln!("Failed to parse config file: {}", e);
                        Self::default()
                    }
                },
                Err(e) => {
                    eprintln!("Failed to read config file: {}", e);
                    Self::default()
                }
            }
        } else {
            Self::default()
        }
    }

    /// Save configuration to disk
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)
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
