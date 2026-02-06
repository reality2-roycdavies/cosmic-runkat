//! cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop
//!
//! Inspired by RunCat, this application displays an animated cat in the
//! COSMIC panel that runs faster when CPU usage is higher.
//!
//! ## Modes
//!
//! - No arguments: Run as a COSMIC panel applet
//! - `--settings` or `-s`: Open the settings window

mod applet;
mod config;
mod constants;
mod cpu;
mod error;
mod paths;
mod settings;
mod sysinfo;
mod theme;

use std::env;

fn print_help() {
    println!(
        r#"cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop

Usage: cosmic-runkat [OPTIONS]

Options:
    -s, --settings   Open the settings window
    -h, --help       Show this help message
    -v, --version    Show version information

No arguments: Run as a COSMIC panel applet.
"#
    );
}

fn print_version() {
    println!("cosmic-runkat {}", env!("CARGO_PKG_VERSION"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("cosmic_runkat=info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting cosmic-runkat v{}", env!("CARGO_PKG_VERSION"));

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
                tracing::info!("Opening settings window");
                settings::run_settings().map_err(|e| e.into())
            }
            arg => {
                eprintln!("Unknown argument: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    } else {
        // Default: run as COSMIC panel applet
        tracing::info!("Starting COSMIC panel applet");
        applet::run_applet().map_err(|e| e.into())
    }
}
