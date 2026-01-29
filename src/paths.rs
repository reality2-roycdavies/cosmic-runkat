//! Path and environment detection
//!
//! Provides Flatpak-aware path resolution for configuration files
//! and utilities for detecting the runtime environment.

use std::path::PathBuf;

/// Check if running inside a Flatpak sandbox
///
/// Detects Flatpak by checking for the presence of `/.flatpak-info`,
/// which is mounted by the Flatpak runtime.
///
/// # Examples
///
/// ```
/// if is_flatpak() {
///     println!("Running in Flatpak sandbox");
/// }
/// ```
pub fn is_flatpak() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Get the app config directory, using host path in Flatpak
///
/// In Flatpak, this returns the host's config directory (accessible via
/// filesystem permissions in the manifest). In native environments, it
/// returns the standard XDG config directory.
///
/// # Returns
///
/// Path to `cosmic-runkat` config directory:
/// - Native: `$XDG_CONFIG_HOME/cosmic-runkat` or `~/.config/cosmic-runkat`
/// - Flatpak: `~/.config/cosmic-runkat` (on host, exposed to sandbox)
///
/// Falls back to `/tmp/cosmic-runkat` if home directory cannot be determined.
///
/// # Examples
///
/// ```
/// let config_dir = app_config_dir();
/// let config_file = config_dir.join("config.json");
/// ```
pub fn app_config_dir() -> PathBuf {
    if is_flatpak() {
        // In Flatpak, use the exposed host config directory
        // This works because the Flatpak manifest grants filesystem access to ~/.config
        dirs::home_dir()
            .map(|h| h.join(".config/cosmic-runkat"))
            .unwrap_or_else(|| PathBuf::from("/tmp/cosmic-runkat"))
    } else {
        // Native: use standard XDG config directory
        dirs::config_dir()
            .map(|d| d.join("cosmic-runkat"))
            .unwrap_or_else(|| PathBuf::from("/tmp/cosmic-runkat"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_flatpak_detection() {
        // We can't actually test this without being in Flatpak,
        // but we can verify the function doesn't panic
        let _ = is_flatpak();
    }

    #[test]
    fn test_app_config_dir_returns_valid_path() {
        let dir = app_config_dir();
        assert!(dir.to_str().is_some());
        assert!(dir.ends_with("cosmic-runkat"));
    }

    #[test]
    fn test_app_config_dir_is_absolute() {
        let dir = app_config_dir();
        assert!(dir.is_absolute() || dir.starts_with("/tmp"));
    }
}
