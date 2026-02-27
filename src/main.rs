//! cosmic-runkat — A cute running cat CPU indicator for COSMIC desktop
//!
//! Inspired by [RunCat](https://kyome.io/runcat/), this application displays
//! an animated cat in the COSMIC panel that runs faster when the CPU is busy.
//!
//! ## How to run
//!
//! - **No arguments**: Starts as a COSMIC panel applet (normal usage)
//! - **`--settings`** or **`-s`**: Opens the standalone settings window
//! - **`--help`**: Shows usage information
//! - **`--version`**: Shows the version number

// Each `mod` declaration tells Rust to include the corresponding source file.
// For example, `mod applet` includes `src/applet.rs`.
mod applet;
mod config;
mod constants;
mod cpu;
mod error;
mod settings;
mod settings_page;
mod sysinfo;
mod theme;

use std::env;

const APPLET_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat";

/// Print command-line usage information
fn print_help() {
    println!(
        r#"cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop

Usage: cosmic-runkat [OPTIONS]

Options:
    -s, --settings           Open settings (via hub or standalone)
    --settings-standalone    Open standalone settings window
    -h, --help               Show this help message
    -v, --version            Show version information

No arguments: Run as a COSMIC panel applet.
"#
    );
}

/// Print the version from Cargo.toml (embedded at compile time)
fn print_version() {
    println!("cosmic-runkat {}", env!("CARGO_PKG_VERSION"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up structured logging using the `tracing` crate.
    // Log level can be overridden with the RUST_LOG environment variable,
    // e.g. RUST_LOG=debug cosmic-runkat
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cosmic_runkat=info".parse().expect("valid log directive")),
        )
        .init();

    tracing::info!("Starting cosmic-runkat v{}", env!("CARGO_PKG_VERSION"));

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_help();
                Ok(())
            }
            "-v" | "--version" => {
                print_version();
                Ok(())
            }
            "-s" | "--settings" => {
                tracing::info!("Opening settings (trying hub first)");
                open_settings().map_err(|e| e.into())
            }
            "--settings-standalone" => {
                tracing::info!("Opening standalone settings window");
                settings::run_settings().map_err(|e| e.into())
            }
            arg => {
                Err(format!("Unknown argument: {}", arg).into())
            }
        }
    } else {
        // Default (no arguments): run as a COSMIC panel applet
        tracing::info!("Starting COSMIC panel applet");
        applet::run_applet().map_err(|e| e.into())
    }
}

/// Try to open settings via cosmic-applet-settings hub; fall back to standalone.
fn open_settings() -> cosmic::iced::Result {
    use std::process::Command;
    if Command::new("cosmic-applet-settings")
        .arg(APPLET_ID)
        .spawn()
        .is_ok()
    {
        Ok(())
    } else {
        settings::run_settings()
    }
}
