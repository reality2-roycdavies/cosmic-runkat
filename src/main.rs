//! cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop
//!
//! Inspired by RunCat, this application displays an animated cat in the system tray
//! that runs faster when CPU usage is higher.
//!
//! ## Modes
//!
//! - `--tray` or `-t`: Run the system tray icon (for autostart)
//! - `--settings` or `-s`: Open the settings window
//! - No arguments: Opens settings (starts tray first if not already running)

mod config;
mod constants;
mod cpu;
mod error;
mod paths;
mod settings;
mod tray;

use constants::*;
use paths::{app_config_dir, is_flatpak};
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;

/// Spawn tray in background (Flatpak-aware)
///
/// In Flatpak, uses flatpak-spawn to launch the app on the host.
/// In native environments, spawns directly.
fn spawn_tray_background() -> Result<(), std::io::Error> {
    if is_flatpak() {
        // In Flatpak, must use flatpak-spawn to launch on host
        Command::new("flatpak-spawn")
            .args([
                "--host",
                "flatpak",
                "run",
                "io.github.reality2_roycdavies.cosmic-runkat",
                "--tray",
            ])
            .spawn()
            .map(|_| ())
    } else {
        // Native: spawn directly
        Command::new(env::current_exe()?)
            .arg("--tray")
            .spawn()
            .map(|_| ())
    }
}

/// Get the path to the tray lockfile
fn tray_lockfile_path() -> std::path::PathBuf {
    app_config_dir().join("tray.lock")
}

/// Get the path to the GUI lockfile
fn gui_lockfile_path() -> std::path::PathBuf {
    app_config_dir().join("gui.lock")
}

fn print_help() {
    println!(
        r#"cosmic-runkat - A cute running cat CPU indicator for COSMIC desktop

Usage: cosmic-runkat [OPTIONS]

Options:
    -t, --tray       Run the system tray icon (for autostart)
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

/// Check if a lockfile indicates an active process
///
/// Checks existence and modification time (to detect stale locks from crashes/restarts)
fn is_lockfile_active(path: &std::path::Path) -> bool {
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(elapsed) = modified.elapsed() {
                // If lockfile was modified recently, process is likely running
                return elapsed < LOCKFILE_STALE_THRESHOLD;
            }
        }
        // If we can't check time, assume NOT running (conservative approach)
        // This prevents stale lockfiles from blocking new instances
        return false;
    }
    false
}

/// Create a lockfile with the current PID
fn create_lockfile(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::File::create(path) {
        let _ = write!(file, "{}", std::process::id());
    }
}

/// Remove a lockfile
fn remove_lockfile(path: &std::path::Path) {
    let _ = fs::remove_file(path);
}

/// Check if cosmic-runkat tray is already running using a lockfile
/// In Flatpak, we can't check /proc/PID due to PID namespace isolation,
/// so we just check if the lockfile exists (with a timestamp check for stale files)
fn is_tray_running() -> bool {
    is_lockfile_active(&tray_lockfile_path())
}

/// Create a lockfile to indicate the tray is running
/// Called at the start of tray mode
pub fn create_tray_lockfile() {
    create_lockfile(&tray_lockfile_path());
}

/// Remove the lockfile when tray exits
pub fn remove_tray_lockfile() {
    remove_lockfile(&tray_lockfile_path());
}

/// Check if the GUI/settings app is already running
fn is_gui_running() -> bool {
    is_lockfile_active(&gui_lockfile_path())
}

/// Clean up stale lockfiles from previous sessions
/// Called at startup to prevent orphaned lockfiles from blocking new instances
fn cleanup_stale_lockfiles() {
    // Clean up stale GUI lockfile
    let gui_lockfile = gui_lockfile_path();
    cleanup_single_lockfile(&gui_lockfile, "GUI");

    // Clean up stale tray lockfile
    let tray_lockfile = tray_lockfile_path();
    cleanup_single_lockfile(&tray_lockfile, "tray");
}

/// Get system boot time as seconds since Unix epoch
fn get_boot_time() -> Option<u64> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    for line in stat.lines() {
        if line.starts_with("btime ") {
            return line.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

/// Helper to clean up a single stale lockfile
fn cleanup_single_lockfile(lockfile: &std::path::Path, name: &str) {
    if let Ok(metadata) = fs::metadata(lockfile) {
        if let Ok(modified) = metadata.modified() {
            // Check if lockfile is from before the current boot
            if let Some(boot_time) = get_boot_time() {
                if let Ok(modified_unix) = modified.duration_since(std::time::UNIX_EPOCH) {
                    if modified_unix.as_secs() < boot_time {
                        let _ = fs::remove_file(lockfile);
                        eprintln!("Cleaned up {} lockfile from previous boot", name);
                        return;
                    }
                }
            }

            // Also check elapsed time for same-boot stale files
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed >= LOCKFILE_STALE_THRESHOLD {
                    let _ = fs::remove_file(lockfile);
                    eprintln!("Cleaned up stale {} lockfile", name);
                }
            }
        } else {
            // Can't check modification time - remove it to be safe
            let _ = fs::remove_file(lockfile);
            eprintln!("Removed {} lockfile with unreadable metadata", name);
        }
    }
}

/// Create a lockfile to indicate the GUI is running
pub fn create_gui_lockfile() {
    create_lockfile(&gui_lockfile_path());
}

/// Remove the GUI lockfile when app exits
pub fn remove_gui_lockfile() {
    remove_lockfile(&gui_lockfile_path());
}

/// Ensure autostart entry exists for the tray
/// Creates an XDG autostart desktop file so the tray starts on login
fn ensure_autostart() {
    let autostart_dir = if is_flatpak() {
        // In Flatpak, write to the host's autostart directory
        dirs::home_dir().map(|h| h.join(".config/autostart"))
    } else {
        dirs::config_dir().map(|d| d.join("autostart"))
    };

    let Some(autostart_dir) = autostart_dir else {
        return;
    };

    let desktop_file = autostart_dir.join("io.github.reality2_roycdavies.cosmic-runkat.desktop");

    // Only create if it doesn't exist (don't overwrite user modifications)
    if desktop_file.exists() {
        return;
    }

    let _ = fs::create_dir_all(&autostart_dir);

    let exec_cmd = if is_flatpak() {
        "flatpak run io.github.reality2_roycdavies.cosmic-runkat --tray"
    } else {
        "cosmic-runkat --tray"
    };

    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=RunKat
Comment=Running cat CPU indicator
Exec={exec_cmd}
Icon=io.github.reality2_roycdavies.cosmic-runkat
Terminal=false
Categories=Utility;
X-GNOME-Autostart-enabled=true
"#
    );

    let _ = fs::write(&desktop_file, content);
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
            "-t" | "--tray" => {
                // Clean up any stale lockfiles from previous sessions
                cleanup_stale_lockfiles();

                // Check if tray is already running
                if is_tray_running() {
                    println!("RunKat tray is already running.");
                    return Ok(());
                }

                println!("Starting cosmic-runkat tray...");
                ensure_autostart();
                tray::run_tray().map_err(|e| e.into())
            }
            "-s" | "--settings" => {
                // Clean up any stale lockfiles from previous sessions
                cleanup_stale_lockfiles();

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
        // Clean up any stale lockfiles from previous sessions first
        cleanup_stale_lockfiles();

        if is_gui_running() {
            println!("Settings window is already open.");
            return Ok(());
        }
        if !is_tray_running() {
            println!("Starting cosmic-runkat tray in background...");
            // Start tray in background (Flatpak-aware)
            if let Err(e) = spawn_tray_background() {
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
