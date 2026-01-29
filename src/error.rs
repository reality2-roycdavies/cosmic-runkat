//! Error types for cosmic-runkat

#![allow(dead_code)] // Error types used in later phases

use thiserror::Error;

/// Main error type for cosmic-runkat operations
#[derive(Debug, Error)]
pub enum RunkatError {
    /// Failed to load image resources
    #[error("Failed to load image resources: {0}")]
    ResourceLoad(#[from] image::ImageError),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Invalid configuration value
    #[error("Invalid configuration: {0}")]
    ConfigValidation(String),

    /// Tray initialization failed
    #[error("Tray initialization failed: {0}")]
    TrayInit(String),

    /// Theme detection/loading error
    #[error("Theme error: {0}")]
    Theme(String),

    /// I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// D-Bus communication error
    #[error("D-Bus error: {0}")]
    DBus(String),
}

/// Convenient result type using RunkatError
pub type Result<T> = std::result::Result<T, RunkatError>;
