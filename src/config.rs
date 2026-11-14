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
/// (e.g. `"cpu-usage"`, `"frequency"`, `"temperature"`, `"memory"`, `"gpu"`).
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
    /// Cat speed reflects RAM usage percentage
    Memory,
    /// Cat speed reflects GPU load (busiest GPU)
    Gpu,
}

impl AnimationSource {
    /// All variants in display order — used for dropdown menus
    pub const ALL: &'static [AnimationSource] = &[
        AnimationSource::CpuUsage,
        AnimationSource::Frequency,
        AnimationSource::Temperature,
        AnimationSource::Memory,
        AnimationSource::Gpu,
    ];

    /// Human-readable names corresponding to `ALL` — shown in the settings UI
    pub const NAMES: &'static [&'static str] = &[
        "CPU Usage",
        "CPU Frequency",
        "CPU Temperature",
        "Memory Usage",
        "GPU Usage",
    ];

    /// True for sources measured as a plain 0-100% load, which can show
    /// the percentage label next to the runner.
    pub fn is_percentage(self) -> bool {
        matches!(
            self,
            AnimationSource::CpuUsage | AnimationSource::Memory | AnimationSource::Gpu
        )
    }
}

/// Which animal runs in the panel.  All runner artwork comes from RunCat365.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Runner {
    #[default]
    Cat,
    Horse,
    Parrot,
}

impl Runner {
    pub const ALL: &'static [Runner] = &[Runner::Cat, Runner::Horse, Runner::Parrot];
    pub const NAMES: &'static [&'static str] = &["Cat", "Horse", "Parrot"];

    pub fn name(self) -> &'static str {
        Self::NAMES[Self::ALL.iter().position(|&r| r == self).unwrap_or(0)]
    }
}

/// How the metric is turned into animation speed.  Set in config.json only
/// (`"speed_curve": "runcat"` or `"linear"`); not shown in the settings UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SpeedCurve {
    /// RunCat365's curve: a slow 2 FPS trot until 5% load, then speed grows
    /// with load up to a 40 FPS sprint at 100%.
    #[default]
    #[serde(rename = "runcat")]
    RunCat,
    /// Straight line from `min_fps` at 0% to `max_fps` at 100%.
    Linear,
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

    /// Below this RAM usage %, the runner sleeps (memory mode only)
    #[serde(default = "default_mem_threshold")]
    pub sleep_threshold_mem: f32,

    /// Below this GPU load %, the runner sleeps (GPU mode only)
    #[serde(default = "default_gpu_threshold")]
    pub sleep_threshold_gpu: f32,

    /// Fastest the animation can run (frames per second), linear curve only
    pub max_fps: f32,

    /// Slowest the animation can run (frames per second), linear curve only
    pub min_fps: f32,

    /// Whether to show "42%" next to the cat in CPU usage mode
    pub show_percentage: bool,

    /// Which system metric drives the animation and popup display
    #[serde(default)]
    pub animation_source: AnimationSource,

    /// Which animal runs in the panel
    #[serde(default)]
    pub runner: Runner,

    /// How the metric maps to animation speed
    #[serde(default)]
    pub speed_curve: SpeedCurve,

    /// Command run when the applet is middle-clicked.  Empty means "find a
    /// system monitor automatically".  Set in config.json only; not shown in
    /// the settings UI.
    #[serde(default)]
    pub middle_click_command: String,
}

// Default value functions for serde — called when a field is missing from
// the JSON file.
fn default_cpu_threshold() -> f32 { 5.0 }
fn default_freq_threshold() -> f32 { 1000.0 }  // 1 GHz
fn default_temp_threshold() -> f32 { 40.0 }     // 40 C
fn default_mem_threshold() -> f32 { 0.0 }       // never sleep on RAM
fn default_gpu_threshold() -> f32 { 5.0 }

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_threshold_cpu: 5.0,
            sleep_threshold_freq: 1000.0,
            sleep_threshold_temp: 40.0,
            sleep_threshold_mem: default_mem_threshold(),
            sleep_threshold_gpu: default_gpu_threshold(),
            max_fps: 15.0,
            min_fps: 2.0,
            show_percentage: true,
            animation_source: AnimationSource::default(),
            runner: Runner::default(),
            speed_curve: SpeedCurve::default(),
            middle_click_command: String::new(),
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
            AnimationSource::Memory => self.sleep_threshold_mem,
            AnimationSource::Gpu => self.sleep_threshold_gpu,
        }
    }

    /// Set the sleep threshold for the currently selected animation source.
    /// Called from the settings window when the user moves the slider.
    pub fn set_current_threshold(&mut self, value: f32) {
        match self.animation_source {
            AnimationSource::CpuUsage => self.sleep_threshold_cpu = value,
            AnimationSource::Frequency => self.sleep_threshold_freq = value,
            AnimationSource::Temperature => self.sleep_threshold_temp = value,
            AnimationSource::Memory => self.sleep_threshold_mem = value,
            AnimationSource::Gpu => self.sleep_threshold_gpu = value,
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

        for (name, v) in [
            ("sleep_threshold_mem", self.sleep_threshold_mem),
            ("sleep_threshold_gpu", self.sleep_threshold_gpu),
        ] {
            if !(0.0..=100.0).contains(&v) {
                return Err(format!("{name} must be between 0 and 100, got {v}"));
            }
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

    /// Calculate how fast the runner should animate based on a 0-100% metric.
    ///
    /// - `RunCat`: RunCat365's formula, `2 * max(1, load / 5)` FPS, which is
    ///   2 FPS up to 5% load and 40 FPS at 100%.
    /// - `Linear`: at 0% -> `min_fps`, at 100% -> `max_fps`.
    ///
    /// Values outside 0-100 are clamped.
    pub fn calculate_fps(&self, percent: f32) -> f32 {
        let percent = percent.clamp(0.0, 100.0);
        match self.speed_curve {
            SpeedCurve::RunCat => 2.0 * (percent / 5.0).max(1.0),
            SpeedCurve::Linear => {
                self.min_fps + percent / 100.0 * (self.max_fps - self.min_fps)
            }
        }
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
        let mut config = Config::default();
        config.speed_curve = SpeedCurve::Linear;

        // At very low CPU, fps should be close to min_fps
        let low_fps = config.calculate_fps(3.0);
        assert!(low_fps >= config.min_fps);
        assert!(low_fps < config.min_fps + 1.0);

        // At 100% CPU = max fps
        assert!((config.calculate_fps(100.0) - config.max_fps).abs() < 0.01);
    }

    #[test]
    fn test_runcat_fps_curve() {
        let config = Config::default();
        assert_eq!(config.speed_curve, SpeedCurve::RunCat);
        assert!((config.calculate_fps(0.0) - 2.0).abs() < 0.01);
        assert!((config.calculate_fps(5.0) - 2.0).abs() < 0.01);
        assert!((config.calculate_fps(50.0) - 20.0).abs() < 0.01);
        assert!((config.calculate_fps(100.0) - 40.0).abs() < 0.01);
        assert!((config.calculate_fps(250.0) - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_old_config_gets_new_defaults() {
        let old = r#"{"sleep_threshold_cpu": 10.0, "max_fps": 15.0, "min_fps": 2.0,
                      "show_percentage": false, "animation_source": "cpu-usage"}"#;
        let config: Config = serde_json::from_str(old).unwrap();
        assert_eq!(config.runner, Runner::Cat);
        assert_eq!(config.speed_curve, SpeedCurve::RunCat);
        assert!(config.middle_click_command.is_empty());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_new_fields_round_trip() {
        let mut config = Config::default();
        config.runner = Runner::Parrot;
        config.animation_source = AnimationSource::Gpu;
        config.speed_curve = SpeedCurve::Linear;
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""runner":"parrot""#));
        assert!(json.contains(r#""animation_source":"gpu""#));
        assert!(json.contains(r#""speed_curve":"linear""#));
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.runner, Runner::Parrot);
        assert_eq!(back.animation_source, AnimationSource::Gpu);
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
