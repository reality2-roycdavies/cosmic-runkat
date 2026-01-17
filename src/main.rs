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
use std::fs;
use std::io::Write;
use std::process::Command;

/// Get the path to the tray lockfile
/// Uses config directory to work correctly in Flatpak sandboxes
fn tray_lockfile_path() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("cosmic-runkat"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("tray.lock")
}

/// Get the path to the GUI lockfile
fn gui_lockfile_path() -> std::path::PathBuf {
    dirs::config_dir()
        .map(|d| d.join("cosmic-runkat"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("gui.lock")
}

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

/// Check if cosmic-runkat tray is already running using a lockfile
/// In Flatpak, we can't check /proc/PID due to PID namespace isolation,
/// so we just check if the lockfile exists (with a timestamp check for stale files)
fn is_tray_running() -> bool {
    let lockfile = tray_lockfile_path();

    if let Ok(metadata) = fs::metadata(&lockfile) {
        // Check if lockfile is recent (less than 1 minute old means tray is likely running)
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                // If lockfile was modified less than 60 seconds ago, tray is running
                return elapsed.as_secs() < 60;
            }
        }
        // If we can't check time, assume running if file exists
        return true;
    }
    false
}

/// Create a lockfile to indicate the tray is running
/// Called at the start of tray mode
pub fn create_tray_lockfile() {
    let lockfile = tray_lockfile_path();
    // Ensure parent directory exists
    if let Some(parent) = lockfile.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::File::create(&lockfile) {
        let _ = write!(file, "{}", std::process::id());
    }
}

/// Remove the lockfile when tray exits
pub fn remove_tray_lockfile() {
    let _ = fs::remove_file(tray_lockfile_path());
}

/// Check if the GUI/settings app is already running
fn is_gui_running() -> bool {
    let lockfile = gui_lockfile_path();

    if let Ok(metadata) = fs::metadata(&lockfile) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                return elapsed.as_secs() < 60;
            }
        }
        return true;
    }
    false
}

/// Create a lockfile to indicate the GUI is running
pub fn create_gui_lockfile() {
    let lockfile = gui_lockfile_path();
    if let Some(parent) = lockfile.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::File::create(&lockfile) {
        let _ = write!(file, "{}", std::process::id());
    }
}

/// Remove the GUI lockfile when app exits
pub fn remove_gui_lockfile() {
    let _ = fs::remove_file(gui_lockfile_path());
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
                if is_gui_running() {
                    println!("Settings window is already open.");
                    return Ok(());
                }
                create_gui_lockfile();
                let result = settings::run_settings().map_err(|e| e.into());
                remove_gui_lockfile();
                result
            }
            arg => {
                eprintln!("Unknown argument: {}", arg);
                print_help();
                std::process::exit(1);
            }
        }
    } else {
        // Default: smart mode - always open settings, start tray first if not running
        if is_gui_running() {
            println!("Settings window is already open.");
            return Ok(());
        }
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
        create_gui_lockfile();
        let result = settings::run_settings().map_err(|e| e.into());
        remove_gui_lockfile();
        result
    }
}
