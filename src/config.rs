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

    /// Maximum animation speed in frames per second (default: 10)
    pub max_fps: f32,

    /// Minimum animation speed in frames per second (default: 1)
    pub min_fps: f32,

    /// Show CPU percentage on the tray icon (default: true)
    pub show_percentage: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_threshold: 5.0,
            max_fps: 10.0,
            min_fps: 1.0,
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
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Self::default(),
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
    pub fn calculate_fps(&self, cpu_percent: f32) -> f32 {
        if cpu_percent < self.sleep_threshold {
            0.0 // Cat is sleeping
        } else {
            // Linear interpolation between min_fps and max_fps
            // based on CPU percentage (threshold to 100%)
            let range = 100.0 - self.sleep_threshold;
            let normalized = (cpu_percent - self.sleep_threshold) / range;
            self.min_fps + normalized * (self.max_fps - self.min_fps)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_calculation() {
        let config = Config::default();

        // Below threshold = sleeping
        assert_eq!(config.calculate_fps(3.0), 0.0);

        // At threshold = min fps
        assert!(config.calculate_fps(5.0) >= config.min_fps);

        // At 100% = max fps
        assert!((config.calculate_fps(100.0) - config.max_fps).abs() < 0.01);
    }
}
