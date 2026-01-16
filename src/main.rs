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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                print_version();
                return Ok(());
            }
            "-d" | "--daemon" => {
                println!("Starting cosmic-runkat daemon...");
                daemon::run_daemon().await?;
                return Ok(());
            }
            "-t" | "--tray" => {
                println!("Starting cosmic-runkat tray...");
                tray::run_tray().map_err(|e| e.into())
            }
            "-s" | "--settings" => {
                println!("Opening settings (not yet implemented)...");
                // TODO: Implement settings app
                Ok(())
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
