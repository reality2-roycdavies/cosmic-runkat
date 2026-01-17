//! System Tray Module
//!
//! Implements an animated system tray icon showing a running cat.
//! The animation speed varies based on CPU usage.
//! CPU percentage is dynamically composited onto the icon.

use image::{DynamicImage, GenericImage, GenericImageView, Rgba, RgbaImage};
use ksni::{Tray, TrayService};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::cpu::CpuMonitor;

/// Number of animation frames in the run cycle
const RUN_FRAMES: u8 = 10;

/// Cat size (square)
const CAT_SIZE: u32 = 32;

/// Digit size (width x height)
const DIGIT_WIDTH: u32 = 8;
const DIGIT_HEIGHT: u32 = 12;

/// Spacing between cat and percentage
const CAT_PCT_SPACING: u32 = 2;

/// Get the path to COSMIC's theme config file
fn cosmic_theme_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cosmic/com.system76.CosmicTheme.Mode/v1/is_dark"))
}

/// Get the path to COSMIC's panel size config file
fn cosmic_panel_size_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cosmic/com.system76.CosmicPanel.Panel/v1/size"))
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
            "call", "--session",
            "--dest", "org.freedesktop.portal.Desktop",
            "--object-path", "/org/freedesktop/portal/desktop",
            "--method", "org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance", "color-scheme"
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

/// Load a digit sprite
fn load_digit(digit: char, is_light: bool) -> Option<RgbaImage> {
    let data: &[u8] = match (digit, is_light) {
        ('0', false) => include_bytes!("../resources/digit-0.png"),
        ('0', true) => include_bytes!("../resources/digit-0-light.png"),
        ('1', false) => include_bytes!("../resources/digit-1.png"),
        ('1', true) => include_bytes!("../resources/digit-1-light.png"),
        ('2', false) => include_bytes!("../resources/digit-2.png"),
        ('2', true) => include_bytes!("../resources/digit-2-light.png"),
        ('3', false) => include_bytes!("../resources/digit-3.png"),
        ('3', true) => include_bytes!("../resources/digit-3-light.png"),
        ('4', false) => include_bytes!("../resources/digit-4.png"),
        ('4', true) => include_bytes!("../resources/digit-4-light.png"),
        ('5', false) => include_bytes!("../resources/digit-5.png"),
        ('5', true) => include_bytes!("../resources/digit-5-light.png"),
        ('6', false) => include_bytes!("../resources/digit-6.png"),
        ('6', true) => include_bytes!("../resources/digit-6-light.png"),
        ('7', false) => include_bytes!("../resources/digit-7.png"),
        ('7', true) => include_bytes!("../resources/digit-7-light.png"),
        ('8', false) => include_bytes!("../resources/digit-8.png"),
        ('8', true) => include_bytes!("../resources/digit-8-light.png"),
        ('9', false) => include_bytes!("../resources/digit-9.png"),
        ('9', true) => include_bytes!("../resources/digit-9-light.png"),
        ('%', false) => include_bytes!("../resources/digit-pct.png"),
        ('%', true) => include_bytes!("../resources/digit-pct-light.png"),
        _ => return None,
    };
    image::load_from_memory(data).ok().map(|i| i.to_rgba8())
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

/// The system tray implementation
#[derive(Debug)]
pub struct RunkatTray {
    /// Flag to signal when the tray should exit
    should_quit: Arc<AtomicBool>,
    /// Channel to signal updates needed
    update_tx: Sender<()>,
    /// Current animation frame (0-4 for running)
    current_frame: u8,
    /// Current CPU percentage
    cpu_percent: f32,
    /// Is the cat sleeping?
    is_sleeping: bool,
    /// Dark mode state
    dark_mode: bool,
    /// Show percentage on icon (user preference)
    show_percentage: bool,
    /// Panel is medium size or larger
    panel_medium_or_larger: bool,
}

impl RunkatTray {
    pub fn new(should_quit: Arc<AtomicBool>, update_tx: Sender<()>, show_percentage: bool) -> Self {
        Self {
            should_quit,
            update_tx,
            current_frame: 0,
            cpu_percent: 0.0,
            is_sleeping: true,
            dark_mode: is_dark_mode(),
            show_percentage,
            panel_medium_or_larger: is_panel_medium_or_larger(),
        }
    }

    /// Get the cat icon data for the current frame
    fn get_cat_data(&self) -> &'static [u8] {
        if self.is_sleeping {
            if self.dark_mode {
                include_bytes!("../resources/cat-sleep-light.png")
            } else {
                include_bytes!("../resources/cat-sleep.png")
            }
        } else {
            match (self.current_frame, self.dark_mode) {
                (0, true) => include_bytes!("../resources/cat-run-0-light.png"),
                (0, false) => include_bytes!("../resources/cat-run-0.png"),
                (1, true) => include_bytes!("../resources/cat-run-1-light.png"),
                (1, false) => include_bytes!("../resources/cat-run-1.png"),
                (2, true) => include_bytes!("../resources/cat-run-2-light.png"),
                (2, false) => include_bytes!("../resources/cat-run-2.png"),
                (3, true) => include_bytes!("../resources/cat-run-3-light.png"),
                (3, false) => include_bytes!("../resources/cat-run-3.png"),
                (4, true) => include_bytes!("../resources/cat-run-4-light.png"),
                (4, false) => include_bytes!("../resources/cat-run-4.png"),
                (5, true) => include_bytes!("../resources/cat-run-5-light.png"),
                (5, false) => include_bytes!("../resources/cat-run-5.png"),
                (6, true) => include_bytes!("../resources/cat-run-6-light.png"),
                (6, false) => include_bytes!("../resources/cat-run-6.png"),
                (7, true) => include_bytes!("../resources/cat-run-7-light.png"),
                (7, false) => include_bytes!("../resources/cat-run-7.png"),
                (8, true) => include_bytes!("../resources/cat-run-8-light.png"),
                (8, false) => include_bytes!("../resources/cat-run-8.png"),
                (9, true) => include_bytes!("../resources/cat-run-9-light.png"),
                (9, false) => include_bytes!("../resources/cat-run-9.png"),
                _ => include_bytes!("../resources/cat-run-0.png"),
            }
        }
    }

    /// Build the composite icon with cat and optionally CPU percentage beside it
    fn build_icon(&self) -> Option<RgbaImage> {
        // Load cat frame
        let cat_data = self.get_cat_data();
        let cat = image::load_from_memory(cat_data).ok()?.to_rgba8();

        // Only show percentage if user enabled AND panel is medium or larger AND cat is awake
        let should_show_pct = self.show_percentage && self.panel_medium_or_larger && !self.is_sleeping;

        if !should_show_pct {
            // For small panels, scale up the cat to use more space (48x48)
            if !self.panel_medium_or_larger {
                let scaled = image::imageops::resize(
                    &cat,
                    48,
                    48,
                    image::imageops::FilterType::Nearest,
                );
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

        // Composite each digit
        let mut x = text_x;
        for ch in cpu_str.chars() {
            if let Some(digit_sprite) = load_digit(ch, self.dark_mode) {
                composite_sprite(&mut icon, &digit_sprite, x, text_y);
                x += char_spacing;
            }
        }

        // Add % symbol
        if let Some(pct_sprite) = load_digit('%', self.dark_mode) {
            composite_sprite(&mut icon, &pct_sprite, x, text_y);
        }

        Some(icon)
    }
}

impl Tray for RunkatTray {
    fn id(&self) -> String {
        "io.github.cosmic-runkat".to_string()
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
                icon_name: "preferences-system".to_string(),
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
                icon_name: "application-exit".to_string(),
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
pub fn run_tray() -> Result<(), String> {
    // Create lockfile to indicate tray is running
    crate::create_tray_lockfile();

    let should_quit = Arc::new(AtomicBool::new(false));
    let (update_tx, update_rx) = channel();

    // Load config
    let config = Config::load();

    let tray = RunkatTray::new(should_quit.clone(), update_tx.clone(), config.show_percentage);

    let service = TrayService::new(tray);
    let handle = service.handle();

    // Spawn the tray service
    service.spawn();

    // Start CPU monitoring
    let cpu_monitor = CpuMonitor::new();
    let mut cpu_rx = cpu_monitor.subscribe();
    cpu_monitor.start(Duration::from_millis(500));

    // Set up file watcher for theme, panel size, and app config changes
    let (config_tx, config_rx) = channel();
    let _watcher = {
        let tx = config_tx.clone();
        let notify_config = NotifyConfig::default()
            .with_poll_interval(Duration::from_secs(1));
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
    const CONFIG_CHECK_INTERVAL: Duration = Duration::from_millis(500);

    // Animation state
    let mut current_frame: u8 = 0;
    let mut last_frame_time = Instant::now();
    let mut current_cpu: f32 = 0.0;
    let mut last_raw_cpu: f32 = -1.0;

    // CPU smoothing - moving average over last 10 actual readings (5 seconds at 500ms sample rate)
    const CPU_SAMPLE_COUNT: usize = 10;
    let mut cpu_samples: VecDeque<f32> = VecDeque::with_capacity(CPU_SAMPLE_COUNT);

    // Main loop
    loop {
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
        if (smoothed_cpu - current_cpu).abs() > 0.5 {
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

        // Also poll app config periodically (inotify isn't always reliable)
        if last_config_check.elapsed() >= CONFIG_CHECK_INTERVAL {
            last_config_check = Instant::now();
            let new_config = Config::load();
            if new_config.show_percentage != config.show_percentage
                || (new_config.sleep_threshold - config.sleep_threshold).abs() > 0.1
            {
                config = new_config;
                config_changed = true;
            }
        }

        // Update tray if anything changed
        if frame_changed || config_changed || (new_cpu - current_cpu).abs() > 1.0 {
            handle.update(|tray| {
                tray.current_frame = current_frame;
                tray.cpu_percent = display_cpu;  // Use rounded value for display
                tray.is_sleeping = is_sleeping;
                if config_changed {
                    tray.dark_mode = is_dark_mode();
                    tray.panel_medium_or_larger = is_panel_medium_or_larger();
                    tray.show_percentage = config.show_percentage;
                }
            });
        }

        // Refresh lockfile timestamp every 30 seconds to indicate we're still running
        static LOCKFILE_REFRESH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
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

    // Clean up lockfile on exit
    crate::remove_tray_lockfile();

    Ok(())
}
