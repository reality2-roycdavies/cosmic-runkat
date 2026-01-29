//! System Tray Module
//!
//! Implements an animated system tray icon showing a running cat.
//! The animation speed varies based on CPU usage.
//! CPU percentage is dynamically composited onto the icon.

use crate::config::Config;
use crate::constants::*;
use crate::cpu::CpuMonitor;
use image::RgbaImage;
use ksni::Tray;
// Import the blocking TrayMethods trait for sync spawn/disable_dbus_name
use ksni::blocking::TrayMethods as BlockingTrayMethods;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Get the host's COSMIC config directory
/// In Flatpak, dirs::config_dir() returns the sandboxed config, not the host's
fn host_cosmic_config_dir() -> Option<PathBuf> {
    // Always use home directory + .config to get host's config
    // This works both in Flatpak (with filesystem access) and native
    dirs::home_dir().map(|h| h.join(".config/cosmic"))
}

/// Get the path to COSMIC's theme mode config file
fn cosmic_theme_path() -> Option<PathBuf> {
    host_cosmic_config_dir().map(|d| d.join("com.system76.CosmicTheme.Mode/v1/is_dark"))
}

/// Get the path to the active theme directory
fn cosmic_theme_dir() -> Option<PathBuf> {
    let is_dark = is_dark_mode();
    let theme_name = if is_dark { "Dark" } else { "Light" };
    host_cosmic_config_dir().map(|d| d.join(format!("com.system76.CosmicTheme.{}/v1", theme_name)))
}

/// Get modification time of theme color files for change detection
fn get_theme_files_mtime() -> Option<std::time::SystemTime> {
    let theme_dir = cosmic_theme_dir()?;
    let accent_path = theme_dir.join("accent");
    let bg_path = theme_dir.join("background");

    // Return the most recent modification time of either file
    let accent_mtime = fs::metadata(&accent_path).ok()?.modified().ok()?;
    let bg_mtime = fs::metadata(&bg_path).ok()?.modified().ok()?;

    Some(accent_mtime.max(bg_mtime))
}

/// Parse a color from COSMIC theme RON format
fn parse_color_from_ron(content: &str, color_name: &str) -> Option<(u8, u8, u8)> {
    // Basic parser for COSMIC theme RON
    // Looks for `color_name: ( red: X, green: Y, blue: Z ... )`

    // Find "color_name:"
    let key = format!("{}:", color_name);
    let rest = content.split(&key).nth(1)?;

    // Find the content inside the next parenthesis group (...)
    let start = rest.find('(')?;
    let end = rest[start..].find(')')?;
    let block = &rest[start + 1..start + end]; // content inside ( )

    let extract = |name: &str| -> Option<f32> {
        // Look for "name: value" or "name:value"
        let name_key = format!("{}:", name);
        let val_part = block.split(&name_key).nth(1)?;
        // Take until comma or end
        let val_str = val_part.split(',').next()?.trim();
        val_str.parse().ok()
    };

    let r = extract("red")?;
    let g = extract("green")?;
    let b = extract("blue")?;

    Some((
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    ))
}

/// Get theme color for the tray icon (foreground color from background.on)
fn get_theme_color() -> (u8, u8, u8) {
    let default_color = (200, 200, 200);

    let theme_dir = match cosmic_theme_dir() {
        Some(dir) => dir,
        None => return default_color,
    };

    let bg_path = theme_dir.join("background");
    if let Ok(content) = fs::read_to_string(&bg_path) {
        parse_color_from_ron(&content, "on").unwrap_or(default_color)
    } else {
        default_color
    }
}

/// Recolor an RGBA image to use the theme color
/// Preserves alpha channel, replaces RGB with theme color
fn recolor_image(img: &RgbaImage, color: (u8, u8, u8)) -> RgbaImage {
    let (r, g, b) = color;
    let mut result = img.to_owned();
    result
        .pixels_mut()
        .filter(|pixel| pixel[3] > 0)
        .for_each(|pixel| {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        });
    result
}

/// Get the path to COSMIC's panel size config file
fn cosmic_panel_size_path() -> Option<PathBuf> {
    host_cosmic_config_dir().map(|d| d.join("com.system76.CosmicPanel.Panel/v1/size"))
}

/// Check if panel size is medium or larger (M, L, XL)
fn is_panel_medium_or_larger() -> bool {
    if let Some(path) = cosmic_panel_size_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            let size = content.trim().to_uppercase();
            // Panel sizes: XS, S, M, L, XL - show percentage for M and above
            return matches!(size.as_str(), "M" | "L" | "XL");
        }
    }
    // Default to showing percentage if we can't detect
    true
}

/// Detect if the system is in dark mode
fn is_dark_mode() -> bool {
    // Try COSMIC's config file first
    if let Some(path) = cosmic_theme_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            return content.trim() == "true";
        }
    }

    // Fall back to freedesktop portal
    if let Ok(output) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",
            "color-scheme",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("uint32 1") {
            return true;
        } else if stdout.contains("uint32 2") {
            return false;
        }
    }

    // Default to dark mode
    true
}

/// Composite a sprite onto a target image at the given position
fn composite_sprite(target: &mut RgbaImage, sprite: &RgbaImage, x: u32, y: u32) {
    for (sx, sy, pixel) in sprite.enumerate_pixels() {
        let tx = x + sx;
        let ty = y + sy;
        if tx < target.width() && ty < target.height() && pixel[3] > 0 {
            target.put_pixel(tx, ty, *pixel);
        }
    }
}

/// Cache for loaded image resources to avoid repeated decoding
struct Resources {
    cat_frames: Vec<RgbaImage>,
    cat_sleep: RgbaImage,
    digits: std::collections::HashMap<char, RgbaImage>,
}

impl std::fmt::Debug for Resources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resources")
            .field("cat_frames", &self.cat_frames.len())
            .finish()
    }
}

impl Resources {
    fn load() -> Option<Self> {
        let load_img = |data: &[u8]| -> Option<RgbaImage> {
            image::load_from_memory(data).ok().map(|i| i.to_rgba8())
        };

        let cat_sleep = load_img(include_bytes!("../resources/cat-sleep.png"))?;

        let cat_frames = vec![
            load_img(include_bytes!("../resources/cat-run-0.png"))?,
            load_img(include_bytes!("../resources/cat-run-1.png"))?,
            load_img(include_bytes!("../resources/cat-run-2.png"))?,
            load_img(include_bytes!("../resources/cat-run-3.png"))?,
            load_img(include_bytes!("../resources/cat-run-4.png"))?,
            load_img(include_bytes!("../resources/cat-run-5.png"))?,
            load_img(include_bytes!("../resources/cat-run-6.png"))?,
            load_img(include_bytes!("../resources/cat-run-7.png"))?,
            load_img(include_bytes!("../resources/cat-run-8.png"))?,
            load_img(include_bytes!("../resources/cat-run-9.png"))?,
        ];

        let mut digits = std::collections::HashMap::new();
        digits.insert('0', load_img(include_bytes!("../resources/digit-0.png"))?);
        digits.insert('1', load_img(include_bytes!("../resources/digit-1.png"))?);
        digits.insert('2', load_img(include_bytes!("../resources/digit-2.png"))?);
        digits.insert('3', load_img(include_bytes!("../resources/digit-3.png"))?);
        digits.insert('4', load_img(include_bytes!("../resources/digit-4.png"))?);
        digits.insert('5', load_img(include_bytes!("../resources/digit-5.png"))?);
        digits.insert('6', load_img(include_bytes!("../resources/digit-6.png"))?);
        digits.insert('7', load_img(include_bytes!("../resources/digit-7.png"))?);
        digits.insert('8', load_img(include_bytes!("../resources/digit-8.png"))?);
        digits.insert('9', load_img(include_bytes!("../resources/digit-9.png"))?);
        digits.insert('%', load_img(include_bytes!("../resources/digit-pct.png"))?);

        Some(Self {
            cat_frames,
            cat_sleep,
            digits,
        })
    }
}

/// Reason for tray exit - used for suspend/resume detection
#[derive(Debug)]
enum TrayExitReason {
    /// User requested quit via menu
    Quit,
    /// Detected suspend/resume, should restart tray
    SuspendResume,
}

/// The system tray implementation
#[derive(Debug)]
pub struct RunkatTray {
    /// Flag to signal when the tray should exit
    should_quit: Arc<AtomicBool>,
    /// Current animation frame (0-9 for running)
    current_frame: u8,
    /// Current CPU percentage
    cpu_percent: f32,
    /// Is the cat sleeping?
    is_sleeping: bool,
    /// Show percentage on icon (user preference)
    show_percentage: bool,
    /// Panel is medium size or larger
    panel_medium_or_larger: bool,
    /// Cached image resources
    resources: Resources,
}

impl RunkatTray {
    pub fn new(should_quit: Arc<AtomicBool>, show_percentage: bool) -> Option<Self> {
        Some(Self {
            should_quit,
            current_frame: 0,
            cpu_percent: 0.0,
            is_sleeping: true,
            show_percentage,
            panel_medium_or_larger: is_panel_medium_or_larger(),
            resources: Resources::load()?,
        })
    }

    /// Build the composite icon with cat and optionally CPU percentage beside it
    fn build_icon(&self) -> Option<RgbaImage> {
        // Get theme color for recoloring sprites
        let theme_color = get_theme_color();

        // Get appropriate cat frame from resources and recolor
        let cat_raw = if self.is_sleeping {
            &self.resources.cat_sleep
        } else {
            &self.resources.cat_frames
                [self.current_frame as usize % self.resources.cat_frames.len()]
        };
        let cat = recolor_image(cat_raw, theme_color);

        // Only show percentage if user enabled AND panel is medium or larger AND cat is awake
        let should_show_pct =
            self.show_percentage && self.panel_medium_or_larger && !self.is_sleeping;

        if !should_show_pct {
            // For small panels, scale up the cat to use more space (48x48)
            if !self.panel_medium_or_larger {
                let scaled =
                    image::imageops::resize(&cat, 48, 48, image::imageops::FilterType::Nearest);
                return Some(scaled);
            }
            // Just return the cat if no percentage
            return Some(cat);
        }

        // Format CPU percentage (no decimal, max 3 chars)
        let cpu_str = format!("{:.0}", self.cpu_percent.min(999.0));

        // Calculate percentage text width
        let char_spacing = DIGIT_WIDTH + 1;
        let pct_width = (cpu_str.len() as u32 * char_spacing) + DIGIT_WIDTH; // digits + % symbol

        // Create wider icon: cat + spacing + percentage
        let total_width = CAT_SIZE + CAT_PCT_SPACING + pct_width;
        let mut icon = RgbaImage::new(total_width, CAT_SIZE);

        // Copy cat to left side
        for (x, y, pixel) in cat.enumerate_pixels() {
            if x < icon.width() && y < icon.height() {
                icon.put_pixel(x, y, *pixel);
            }
        }

        // Position percentage text to the right of the cat, vertically centered
        let text_x = CAT_SIZE + CAT_PCT_SPACING;
        let text_y = (CAT_SIZE - DIGIT_HEIGHT) / 2; // Center vertically

        // Composite each digit (recolored with theme color)
        let mut x = text_x;
        for ch in cpu_str.chars() {
            if let Some(digit_raw) = self.resources.digits.get(&ch) {
                let digit_sprite = recolor_image(digit_raw, theme_color);
                composite_sprite(&mut icon, &digit_sprite, x, text_y);
                x += char_spacing;
            }
        }

        // Add % symbol
        if let Some(pct_raw) = self.resources.digits.get(&'%') {
            let pct_sprite = recolor_image(pct_raw, theme_color);
            composite_sprite(&mut icon, &pct_sprite, x, text_y);
        }

        Some(icon)
    }
}

impl Tray for RunkatTray {
    // Show menu on left-click (same as right-click)
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "io.github.reality2_roycdavies.cosmic-runkat".to_string()
    }

    fn icon_theme_path(&self) -> String {
        dirs::data_dir()
            .map(|p| p.join("icons").to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let img = match self.build_icon() {
            Some(img) => img,
            None => return vec![],
        };

        // Convert RGBA to ARGB (network byte order for D-Bus)
        let mut argb_data = Vec::with_capacity((img.width() * img.height() * 4) as usize);
        for pixel in img.pixels() {
            let [r, g, b, a] = pixel.0;
            argb_data.push(a);
            argb_data.push(r);
            argb_data.push(g);
            argb_data.push(b);
        }

        vec![ksni::Icon {
            width: img.width() as i32,
            height: img.height() as i32,
            data: argb_data,
        }]
    }

    fn title(&self) -> String {
        format!("RunKat - CPU: {:.0}%", self.cpu_percent)
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = if self.is_sleeping {
            format!("CPU: {:.0}% (sleeping)", self.cpu_percent)
        } else {
            format!("CPU: {:.0}%", self.cpu_percent)
        };
        ksni::ToolTip {
            title: "RunKat".to_string(),
            description: status,
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        vec![
            StandardItem {
                label: "Settings...".to_string(),
                icon_name: "preferences-system-symbolic".to_string(),
                activate: Box::new(|_| {
                    std::thread::spawn(|| {
                        let exe = std::env::current_exe().unwrap_or_default();
                        let _ = Command::new(exe).arg("--settings").spawn();
                    });
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit-symbolic".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    tray.should_quit.store(true, Ordering::SeqCst);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Starts the system tray service with animated icon
///
/// The tray automatically restarts after suspend/resume to recover from
/// stale D-Bus connections that cause the icon to disappear.
pub fn run_tray() -> Result<(), String> {
    // Brief delay on startup to ensure StatusNotifierWatcher is ready
    // This helps when autostarting at login before the panel is fully initialized
    std::thread::sleep(STARTUP_DELAY);

    // Outer retry loop - restarts tray after suspend/resume
    loop {
        match run_tray_inner()? {
            TrayExitReason::Quit => break,
            TrayExitReason::SuspendResume => {
                println!("Detected suspend/resume, restarting tray...");
                // Brief delay before restarting to let D-Bus settle
                std::thread::sleep(SUSPEND_RESTART_DELAY);
                continue;
            }
        }
    }

    Ok(())
}

/// Inner implementation of the tray service
/// Returns the reason for exit so the outer loop can decide whether to restart
fn run_tray_inner() -> Result<TrayExitReason, String> {
    // Create lockfile to indicate tray is running
    crate::create_tray_lockfile();

    let should_quit = Arc::new(AtomicBool::new(false));

    // Load config
    let config = Config::load();

    let tray = RunkatTray::new(should_quit.clone(), config.show_percentage)
        .ok_or_else(|| "Failed to load tray resources".to_string())?;

    // Spawn the tray service
    // In Flatpak, disable D-Bus well-known name to avoid PID conflicts
    let is_sandboxed = std::path::Path::new("/.flatpak-info").exists();
    let handle = BlockingTrayMethods::disable_dbus_name(tray, is_sandboxed)
        .spawn()
        .map_err(|e| format!("Failed to spawn tray: {}", e))?;

    // Start CPU monitoring
    let cpu_monitor = CpuMonitor::new();
    cpu_monitor.start(CPU_SAMPLE_INTERVAL);

    // Set up file watcher for theme, panel size, and app config changes
    let (config_tx, config_rx) = channel();
    let _watcher = {
        let tx = config_tx.clone();
        let notify_config = NotifyConfig::default().with_poll_interval(Duration::from_secs(1));
        let mut watcher: Result<RecommendedWatcher, _> = Watcher::new(
            move |res: Result<notify::Event, _>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                    ) {
                        let _ = tx.send(());
                    }
                }
            },
            notify_config,
        );
        if let Ok(ref mut w) = watcher {
            // Watch theme config directory
            if let Some(theme_path) = cosmic_theme_path() {
                if let Some(watch_dir) = theme_path.parent() {
                    let _ = w.watch(watch_dir, RecursiveMode::NonRecursive);
                }
            }
            // Watch theme color files directory (accent, background)
            if let Some(theme_dir) = cosmic_theme_dir() {
                let _ = w.watch(&theme_dir, RecursiveMode::NonRecursive);
            }
            // Watch panel config directory
            if let Some(panel_path) = cosmic_panel_size_path() {
                if let Some(watch_dir) = panel_path.parent() {
                    let _ = w.watch(watch_dir, RecursiveMode::NonRecursive);
                }
            }
            // Watch app config directory for settings changes
            let app_config_path = Config::config_path();
            if let Some(watch_dir) = app_config_path.parent() {
                let _ = w.watch(watch_dir, RecursiveMode::NonRecursive);
            }
        }
        watcher.ok()
    };

    // Track current config for detecting changes
    let mut config = config;
    let mut last_config_check = Instant::now();
    let mut tracked_theme_mtime = get_theme_files_mtime();

    // Animation state
    let mut current_frame: u8 = 0;
    let mut last_frame_time = Instant::now();
    let mut current_cpu: f32 = 0.0;
    let mut last_raw_cpu: f32 = -1.0;

    // CPU smoothing - moving average over last N actual readings (5 seconds at 500ms sample rate)
    let mut cpu_samples: VecDeque<f32> = VecDeque::with_capacity(CPU_SAMPLE_COUNT);

    // Track time for suspend/resume detection
    let mut loop_start = Instant::now();

    // Main loop
    loop {
        // Detect suspend/resume by checking for time jumps
        // If the sleep took much longer than expected (>5 seconds vs expected 16ms),
        // we likely woke from suspend and should restart to recover D-Bus connections
        let elapsed = loop_start.elapsed();
        if elapsed > SUSPEND_RESUME_THRESHOLD {
            println!("Time jump detected ({:?}), likely suspend/resume", elapsed);
            handle.shutdown();
            std::thread::sleep(DBUS_CLEANUP_DELAY);
            crate::remove_tray_lockfile();
            return Ok(TrayExitReason::SuspendResume);
        }
        loop_start = Instant::now();

        if should_quit.load(Ordering::SeqCst) {
            break;
        }

        // Check for CPU updates - only add to samples when we get a new reading
        let new_cpu = cpu_monitor.current();
        if (new_cpu - last_raw_cpu).abs() > 0.01 {
            // New reading from monitor
            last_raw_cpu = new_cpu;
            cpu_samples.push_back(new_cpu);
            if cpu_samples.len() > CPU_SAMPLE_COUNT {
                cpu_samples.pop_front();
            }
        }

        // Calculate smoothed average
        let smoothed_cpu = if cpu_samples.is_empty() {
            new_cpu
        } else {
            cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
        };

        // Only update displayed value if change is significant
        if (smoothed_cpu - current_cpu).abs() > CPU_DISPLAY_THRESHOLD {
            current_cpu = smoothed_cpu;
        }

        // Round CPU for display and comparison (user sees whole numbers)
        let display_cpu = current_cpu.round();

        // Calculate animation speed based on actual CPU
        let fps = config.calculate_fps(current_cpu);
        // Sleep check uses rounded value: cat sleeps at 0 to (threshold-1)%
        // e.g., threshold=5 means sleep at 0-4%, run at 5%+
        let is_sleeping = display_cpu < config.sleep_threshold;

        // Update animation frame if running
        let frame_changed = if !is_sleeping && fps > 0.0 {
            let frame_duration = Duration::from_secs_f32(1.0 / fps);
            if last_frame_time.elapsed() >= frame_duration {
                current_frame = (current_frame + 1) % RUN_FRAMES;
                last_frame_time = Instant::now();
                true
            } else {
                false
            }
        } else {
            false
        };

        // Check for config changes (theme, panel size, or app settings)
        let mut config_changed = config_rx.try_recv().is_ok();

        // Also poll app config and theme periodically (inotify isn't always reliable)
        // This runs every ~500ms
        if last_config_check.elapsed() >= CONFIG_CHECK_INTERVAL {
            last_config_check = Instant::now();
            let new_config = Config::load();
            if new_config.show_percentage != config.show_percentage
                || (new_config.sleep_threshold - config.sleep_threshold).abs() > 0.1
            {
                config = new_config;
                config_changed = true;
            }
            // Also check if theme color files have changed (robust backup to file watcher)
            let new_mtime = get_theme_files_mtime();
            if new_mtime != tracked_theme_mtime {
                tracked_theme_mtime = new_mtime;
                config_changed = true;
            }
        }

        // Update tray if anything changed
        if frame_changed || config_changed || (new_cpu - current_cpu).abs() > 1.0 {
            handle.update(|tray| {
                tray.current_frame = current_frame;
                tray.cpu_percent = display_cpu; // Use rounded value for display
                tray.is_sleeping = is_sleeping;
                if config_changed {
                    tray.panel_medium_or_larger = is_panel_medium_or_larger();
                    tray.show_percentage = config.show_percentage;
                }
            });
        } else if last_config_check.elapsed() < Duration::from_millis(20) {
            // Just did a config check - also check if panel size changed without config change
            let new_panel = is_panel_medium_or_larger();
            handle.update(|tray| {
                if tray.panel_medium_or_larger != new_panel {
                    tray.panel_medium_or_larger = new_panel;
                }
            });
        }

        // Refresh lockfile timestamp every 30 seconds to indicate we're still running
        static LOCKFILE_REFRESH: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let last_refresh = LOCKFILE_REFRESH.load(std::sync::atomic::Ordering::Relaxed);
        if now - last_refresh >= 30 {
            crate::create_tray_lockfile();
            LOCKFILE_REFRESH.store(now, std::sync::atomic::Ordering::Relaxed);
        }

        // Sleep briefly - 16ms for ~60Hz update check rate
        std::thread::sleep(Duration::from_millis(16));
    }

    handle.shutdown();

    // Small delay to ensure ksni's D-Bus resources are released
    // Without this, the StatusNotifierItem might briefly appear "stuck"
    std::thread::sleep(DBUS_CLEANUP_DELAY);

    // Clean up lockfile on exit
    crate::remove_tray_lockfile();

    Ok(TrayExitReason::Quit)
}
