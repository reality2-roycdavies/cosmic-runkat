//! Error types for cosmic-runkat
//!
//! Defines a unified error enum for the application.  Currently not wired
//! into all call sites (many functions use ad-hoc error types), but provides
//! a foundation for future error handling improvements.

use thiserror::Error;

/// Main error type for cosmic-runkat operations.
///
/// Each variant wraps a different kind of error that can occur in the
/// application.  The `#[from]` attributes allow automatic conversion
/// from the underlying error types using the `?` operator.
// TODO: wire these variants into call sites that currently use ad-hoc errors
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum RunkatError {
    /// Failed to load or decode an image resource (e.g. a cat sprite PNG)
    #[error("Failed to load image resources: {0}")]
    ResourceLoad(#[from] image::ImageError),

    /// General configuration error (e.g. file not found)
    #[error("Configuration error: {0}")]
    Config(String),

    /// A config value is outside its valid range
    #[error("Invalid configuration: {0}")]
    ConfigValidation(String),

    /// Failed to detect or parse the COSMIC desktop theme
    #[error("Theme error: {0}")]
    Theme(String),

    /// Filesystem I/O error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenient result type alias — avoids writing
/// `Result<T, RunkatError>` everywhere.
#[allow(dead_code)]
pub type Result<T> = std::result::Result<T, RunkatError>;
