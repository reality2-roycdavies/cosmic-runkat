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
//! - No arguments: Smart mode (settings if tray running, otherwise start tray)

mod config;
mod cpu;
mod daemon;
mod dbus_client;
mod settings;
mod tray;

use std::env;
use std::process::Command;

fn print_help() {
    println!(
        r#"cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop

Usage: cosmic-runkat [OPTIONS]

Options:
    -d, --daemon     Run the D-Bus daemon (background service)
    -t, --tray       Run the system tray icon
    -s, --settings   Open the settings window
    -h, --help       Show this help message
    -v, --version    Show version information

No arguments: Opens settings (starts tray first if not already running).
"#
    );
}

fn print_version() {
    println!("cosmic-runkat {}", env!("CARGO_PKG_VERSION"));
}

/// Check if cosmic-runkat tray is already running
fn is_tray_running() -> bool {
    // Use pgrep to check for running tray process
    if let Ok(output) = Command::new("pgrep")
        .args(["-f", "cosmic-runkat.*--tray"])
        .output()
    {
        if output.status.success() {
            // Found at least one process, but exclude ourselves
            let pids = String::from_utf8_lossy(&output.stdout);
            let our_pid = std::process::id().to_string();
            // Check if any PID other than ours is running
            for pid in pids.lines() {
                if pid.trim() != our_pid {
                    return true;
                }
            }
        }
    }
    false
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
        // Default: smart mode - always open settings, start tray first if not running
        if !is_tray_running() {
            println!("Starting cosmic-runkat tray in background...");
            // Start tray in background
            if let Err(e) = Command::new(env::current_exe().unwrap_or_else(|_| "cosmic-runkat".into()))
                .arg("--tray")
                .spawn()
            {
                eprintln!("Warning: Failed to start tray: {}", e);
            }
            // Give tray time to initialize
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        println!("Opening settings...");
        settings::run_settings().map_err(|e| e.into())
    }
}
