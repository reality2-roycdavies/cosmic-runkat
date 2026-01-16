//! cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop
//!
//! Inspired by RunCat, this application displays an animated cat in the system tray
//! that runs faster when CPU usage is higher.
//!
//! ## Modes
//!
//! - `--daemon` or `-d`: Run the D-Bus daemon (background service)
//! - `--tray` or `-t`: Run the system tray icon
//! - `--settings` or `-s`: Open the settings window
//! - No arguments: Run the tray (default)

mod config;
mod cpu;
mod daemon;
mod dbus_client;
mod settings;
mod tray;

use std::env;

fn print_help() {
    println!(
        r#"cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop

Usage: cosmic-runkat [OPTIONS]

Options:
    -d, --daemon     Run the D-Bus daemon (background service)
    -t, --tray       Run the system tray icon (default)
    -s, --settings   Open the settings window
    -h, --help       Show this help message
    -v, --version    Show version information
"#
    );
}

fn print_version() {
    println!("cosmic-runkat {}", env!("CARGO_PKG_VERSION"));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
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
            "-d" | "--daemon" => {
                println!("Starting cosmic-runkat daemon...");
                // Run daemon with tokio runtime
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?
                    .block_on(daemon::run_daemon())?;
                Ok(())
            }
            "-t" | "--tray" => {
                println!("Starting cosmic-runkat tray...");
                tray::run_tray().map_err(|e| e.into())
            }
            "-s" | "--settings" => {
                // Settings uses libcosmic which has its own runtime
                settings::run_settings().map_err(|e| e.into())
            }
            arg => {
                eprintln!("Unknown argument: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    } else {
        // Default: run tray
        println!("Starting cosmic-runkat tray...");
        tray::run_tray().map_err(|e| e.into())
    }
}
