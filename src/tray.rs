//! System Tray Module
//!
//! Implements an animated system tray icon showing a running cat.
//! The animation speed varies based on CPU usage.
//! CPU percentage is dynamically composited onto the icon.

use crate::config::{AnimationSource, Config};
use crate::constants::*;
use crate::cpu::CpuMonitor;
use crate::sysinfo::{CpuFrequency, CpuTemperature};
use crate::theme;
use image::RgbaImage;
use ksni::Tray;
// Using native async API (Phase 3)
use ksni::TrayMethods;
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration, Instant};

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
/// Get theme color for the tray icon (uses theme module)
fn get_theme_color() -> (u8, u8, u8) {
    theme::get_cosmic_theme_colors().foreground
}

/// Create a fallback icon when resources fail to load
///
/// Generates a simple filled circle as a minimal tray icon.
/// Used as graceful degradation when sprite files cannot be loaded.
fn create_fallback_icon(size: u32, color: (u8, u8, u8)) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let (r, g, b) = color;

    // Draw a simple filled circle
    let center = size as f32 / 2.0;
    let radius = size as f32 / 2.5;

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist <= radius {
            *pixel = image::Rgba([r, g, b, 255]);
        }
    }

    img
}

/// Recolor an RGBA image to use the theme color
///
/// Preserves alpha channel, replaces RGB with theme color.
/// Optimized to avoid filtering overhead by checking alpha inline.
fn recolor_image(img: &RgbaImage, color: (u8, u8, u8)) -> RgbaImage {
    let (r, g, b) = color;
    let mut result = img.clone(); // More explicit than to_owned()

    // Mutate in place - more efficient without intermediate filter iterator
    for pixel in result.pixels_mut() {
        if pixel[3] > 0 {
            // Only recolor non-transparent pixels
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }

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
///
/// Skips fully transparent pixels for performance.
/// Boundary checks prevent panics when sprite extends beyond target.
fn composite_sprite(target: &mut RgbaImage, sprite: &RgbaImage, x: u32, y: u32) {
    let target_width = target.width();
    let target_height = target.height();

    for (sx, sy, pixel) in sprite.enumerate_pixels() {
        // Skip fully transparent pixels early
        if pixel[3] == 0 {
            continue;
        }

        let tx = x + sx;
        let ty = y + sy;

        // Bounds check
        if tx < target_width && ty < target_height {
            target.put_pixel(tx, ty, *pixel);
        }
    }
}

/// Cache for loaded image resources to avoid repeated decoding
///
/// Stores both original sprites and recolored versions. The recolored
/// cache is updated only when the theme color changes, avoiding repeated
/// recoloring operations in the render loop.
struct Resources {
    // Original sprites (never modified after load)
    cat_frames_original: Vec<RgbaImage>,
    cat_sleep_original: RgbaImage,
    digits_original: std::collections::HashMap<char, RgbaImage>,

    // Cached recolored sprites (updated only on theme change)
    last_theme_color: Option<(u8, u8, u8)>,
    cat_frames_colored: Vec<RgbaImage>,
    cat_sleep_colored: RgbaImage,
    digits_colored: std::collections::HashMap<char, RgbaImage>,
}

impl std::fmt::Debug for Resources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resources")
            .field("cat_frames", &self.cat_frames_original.len())
            .field("cached_color", &self.last_theme_color)
            .finish()
    }
}

impl Resources {
    fn load() -> Option<Self> {
        let load_img = |data: &[u8]| -> Option<RgbaImage> {
            image::load_from_memory(data).ok().map(|i| i.to_rgba8())
        };

        let cat_sleep_original = load_img(include_bytes!("../resources/cat-sleep.png"))?;

        let cat_frames_original = vec![
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

        let mut digits_original = std::collections::HashMap::new();
        digits_original.insert('0', load_img(include_bytes!("../resources/digit-0.png"))?);
        digits_original.insert('1', load_img(include_bytes!("../resources/digit-1.png"))?);
        digits_original.insert('2', load_img(include_bytes!("../resources/digit-2.png"))?);
        digits_original.insert('3', load_img(include_bytes!("../resources/digit-3.png"))?);
        digits_original.insert('4', load_img(include_bytes!("../resources/digit-4.png"))?);
        digits_original.insert('5', load_img(include_bytes!("../resources/digit-5.png"))?);
        digits_original.insert('6', load_img(include_bytes!("../resources/digit-6.png"))?);
        digits_original.insert('7', load_img(include_bytes!("../resources/digit-7.png"))?);
        digits_original.insert('8', load_img(include_bytes!("../resources/digit-8.png"))?);
        digits_original.insert('9', load_img(include_bytes!("../resources/digit-9.png"))?);
        digits_original.insert('%', load_img(include_bytes!("../resources/digit-pct.png"))?);

        // Clone originals for initial colored cache (will be updated on first theme load)
        Some(Self {
            cat_frames_original: cat_frames_original.clone(),
            cat_sleep_original: cat_sleep_original.clone(),
            digits_original: digits_original.clone(),
            last_theme_color: None,
            cat_frames_colored: cat_frames_original,
            cat_sleep_colored: cat_sleep_original,
            digits_colored: digits_original,
        })
    }

    /// Load resources with fallback on failure
    ///
    /// Returns minimal fallback resources if sprite loading fails,
    /// ensuring the tray can always start even if resources are corrupted.
    fn load_or_fallback() -> Self {
        match Self::load() {
            Some(resources) => {
                tracing::debug!("Loaded sprite resources successfully");
                resources
            }
            None => {
                tracing::error!("Failed to load sprite resources, using fallback");
                Self::create_fallback()
            }
        }
    }

    /// Create minimal fallback resources
    fn create_fallback() -> Self {
        let default_color = (200, 200, 200);
        let fallback_icon = create_fallback_icon(CAT_SIZE, default_color);

        Self {
            cat_frames_original: vec![fallback_icon.clone(); RUN_FRAMES as usize],
            cat_sleep_original: fallback_icon.clone(),
            digits_original: std::collections::HashMap::new(), // No digits in fallback
            last_theme_color: Some(default_color),
            cat_frames_colored: vec![fallback_icon.clone(); RUN_FRAMES as usize],
            cat_sleep_colored: fallback_icon.clone(),
            digits_colored: std::collections::HashMap::new(),
        }
    }

    /// Update cached recolored images if theme changed
    ///
    /// Only recolors sprites when theme color actually changes.
    /// This is called from the main loop when a theme change is detected.
    fn update_colors(&mut self, new_color: (u8, u8, u8)) {
        // Skip recoloring if theme color hasn't changed
        if self.last_theme_color == Some(new_color) {
            return;
        }

        // Recolor all sprites with new theme color
        self.cat_frames_colored =
            self.cat_frames_original.iter().map(|img| recolor_image(img, new_color)).collect();

        self.cat_sleep_colored = recolor_image(&self.cat_sleep_original, new_color);

        self.digits_colored = self
            .digits_original
            .iter()
            .map(|(ch, img)| (*ch, recolor_image(img, new_color)))
            .collect();

        self.last_theme_color = Some(new_color);
    }

    /// Get colored cat frame (uses cache, no recoloring)
    fn get_cat_frame(&self, frame: u8, sleeping: bool) -> &RgbaImage {
        if sleeping {
            &self.cat_sleep_colored
        } else {
            &self.cat_frames_colored[frame as usize % self.cat_frames_colored.len()]
        }
    }

    /// Get colored digit sprite (uses cache, no recoloring)
    fn get_digit(&self, ch: char) -> Option<&RgbaImage> {
        self.digits_colored.get(&ch)
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
    /// Per-core CPU percentages
    per_core_cpu: Vec<f32>,
    /// Current animation metric (0-100, based on animation_source)
    animation_metric: f32,
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
    pub fn new(should_quit: Arc<AtomicBool>, show_percentage: bool) -> Self {
        // Use load_or_fallback to ensure we always succeed
        let mut resources = Resources::load_or_fallback();

        // Initialize with current theme colors
        let initial_color = get_theme_color();
        resources.update_colors(initial_color);

        Self {
            should_quit,
            current_frame: 0,
            cpu_percent: 0.0,
            per_core_cpu: Vec::new(),
            animation_metric: 0.0,
            is_sleeping: true,
            show_percentage,
            panel_medium_or_larger: is_panel_medium_or_larger(),
            resources,
        }
    }

    /// Build the composite icon with cat and optionally CPU percentage beside it
    ///
    /// Uses cached recolored sprites for performance. Theme color updates happen
    /// in the main loop via `update_colors()`, not during rendering.
    fn build_icon(&self) -> Option<RgbaImage> {
        // Get cached colored cat frame (no recoloring!)
        let cat = self.resources.get_cat_frame(self.current_frame, self.is_sleeping);

        // Only show percentage if user enabled AND panel is medium or larger AND cat is awake
        let should_show_pct =
            self.show_percentage && self.panel_medium_or_larger && !self.is_sleeping;

        if !should_show_pct {
            // For small panels, scale up the cat to use more space (48x48)
            if !self.panel_medium_or_larger {
                let scaled =
                    image::imageops::resize(cat, 48, 48, image::imageops::FilterType::Nearest);
                return Some(scaled);
            }
            // Just return the cat if no percentage (clone needed since we return owned)
            return Some(cat.clone());
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

        // Composite each digit (uses cached colored sprites)
        let mut x = text_x;
        for ch in cpu_str.chars() {
            if let Some(digit_sprite) = self.resources.get_digit(ch) {
                composite_sprite(&mut icon, digit_sprite, x, text_y);
                x += char_spacing;
            }
        }

        // Add % symbol
        if let Some(pct_sprite) = self.resources.get_digit('%') {
            composite_sprite(&mut icon, pct_sprite, x, text_y);
        }

        Some(icon)
    }
}

impl Tray for RunkatTray {
    // Don't show menu on left-click - we'll open popup window instead
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        "io.github.reality2_roycdavies.cosmic-runkat".to_string()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // Left-click: Open popup window with CPU details
        // Note: x,y are always 0,0 on Wayland (no global coordinates available)

        // Check if popup is already running by checking processes
        if crate::popup::is_popup_running() {
            return;
        }

        // Spawn popup
        std::thread::spawn(|| {
            let exe = std::env::current_exe().unwrap_or_default();
            let _ = Command::new(exe).arg("--popup").spawn();
        });
    }

    fn icon_theme_path(&self) -> String {
        dirs::data_dir().map(|p| p.join("icons").to_string_lossy().to_string()).unwrap_or_default()
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

        vec![ksni::Icon { width: img.width() as i32, height: img.height() as i32, data: argb_data }]
    }

    fn title(&self) -> String {
        format!("RunKat - CPU: {:.0}%", self.cpu_percent)
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let mut status = if self.is_sleeping {
            format!("CPU: {:.0}% (sleeping)", self.cpu_percent)
        } else {
            format!("CPU: {:.0}%", self.cpu_percent)
        };

        // Add per-core breakdown if available
        if !self.per_core_cpu.is_empty() {
            status.push_str("\n\nPer core:");
            for (i, &pct) in self.per_core_cpu.iter().enumerate() {
                status.push_str(&format!("\n  CPU{}: {:>5.1}%", i, pct));
            }
        }

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
                label: "View Details...".to_string(),
                activate: Box::new(|_| {
                    std::thread::spawn(|| {
                        let exe = std::env::current_exe().unwrap_or_default();
                        let _ = Command::new(exe).arg("--popup").spawn();
                    });
                }),
                ..Default::default()
            }
            .into(),
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
///
/// This now uses a tokio runtime for async event-driven architecture.
pub fn run_tray() -> Result<(), String> {
    // Create tokio runtime for tray (settings app already has one)
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;

    runtime.block_on(async {
        // Brief delay on startup to ensure StatusNotifierWatcher is ready
        tokio::time::sleep(STARTUP_DELAY).await;

        // Outer retry loop - restarts tray after suspend/resume
        loop {
            match run_tray_inner().await? {
                TrayExitReason::Quit => break,
                TrayExitReason::SuspendResume => {
                    tracing::warn!("Detected suspend/resume, restarting tray...");
                    tokio::time::sleep(SUSPEND_RESTART_DELAY).await;
                    continue;
                }
            }
        }

        Ok(())
    })
}

/// Inner implementation of the tray service (async)
/// Returns the reason for exit so the outer loop can decide whether to restart
async fn run_tray_inner() -> Result<TrayExitReason, String> {
    // Create lockfile to indicate tray is running
    crate::create_tray_lockfile();

    let should_quit = Arc::new(AtomicBool::new(false));

    // Load config
    let config = Config::load();

    let tray = RunkatTray::new(should_quit.clone(), config.show_percentage);

    // Spawn the tray service (ASYNC API!)
    // In Flatpak, disable D-Bus well-known name to avoid PID conflicts
    let is_sandboxed = std::path::Path::new("/.flatpak-info").exists();
    tracing::debug!("Spawning tray service (sandboxed: {})", is_sandboxed);

    let handle =
        if is_sandboxed { tray.disable_dbus_name(true).spawn().await } else { tray.spawn().await }
            .map_err(|e| format!("Failed to spawn tray: {}", e))?;

    tracing::info!("Tray service started successfully");

    // Start CPU monitoring
    let cpu_monitor = CpuMonitor::new();
    cpu_monitor.start(CPU_SAMPLE_INTERVAL);
    let mut cpu_rx = cpu_monitor.subscribe();
    let mut cpu_full_rx = cpu_monitor.subscribe_full();

    // Note: File watcher removed - using periodic polling as it's more reliable
    // for theme and config changes. Can be re-added later with async notify if needed.

    // Track current config for detecting changes
    let mut config = config;
    let mut tracked_theme_mtime = get_theme_files_mtime();

    // Animation state
    let mut current_frame: u8 = 0;
    let mut last_frame_time = Instant::now();
    let mut current_cpu: f32 = 0.0;
    let mut last_raw_cpu: f32 = -1.0;

    // CPU smoothing - moving average over last N actual readings
    let mut cpu_samples: VecDeque<f32> = VecDeque::with_capacity(CPU_SAMPLE_COUNT);

    // Async timers for event-driven architecture
    let mut animation_tick = interval(Duration::from_millis(33)); // ~30fps check rate (was 60fps)
    let mut config_check = interval(CONFIG_CHECK_INTERVAL);
    let mut lockfile_refresh = interval(LOCKFILE_REFRESH_INTERVAL);

    // Track time for suspend/resume detection
    let mut loop_start = Instant::now();

    // Main event loop (async with tokio::select!)
    loop {
        tokio::select! {
            // CPU update event (aggregate)
            Ok(_) = cpu_rx.changed() => {
                let new_cpu = *cpu_rx.borrow();

                // Only process if significantly different
                if (new_cpu - last_raw_cpu).abs() > 0.01 {
                    last_raw_cpu = new_cpu;
                    cpu_samples.push_back(new_cpu);
                    if cpu_samples.len() > CPU_SAMPLE_COUNT {
                        cpu_samples.pop_front();
                    }

                    // Calculate smoothed average
                    let smoothed_cpu = if cpu_samples.is_empty() {
                        new_cpu
                    } else {
                        cpu_samples.iter().sum::<f32>() / cpu_samples.len() as f32
                    };

                    // Update displayed value if change is significant
                    if (smoothed_cpu - current_cpu).abs() > CPU_DISPLAY_THRESHOLD {
                        current_cpu = smoothed_cpu;
                        let display_cpu = current_cpu.round();

                        tracing::debug!("CPU: {:.1}%", display_cpu);

                        // Only update cpu_percent for display, is_sleeping is set in animation tick
                        handle.update(|tray| {
                            tray.cpu_percent = display_cpu;
                        }).await;
                    }
                }
            }

            // Per-core CPU update event
            Ok(_) = cpu_full_rx.changed() => {
                let cpu_usage = cpu_full_rx.borrow().clone();
                if !cpu_usage.per_core.is_empty() {
                    handle.update(|tray| {
                        tray.per_core_cpu = cpu_usage.per_core;
                    }).await;
                }
            }

            // Animation tick event
            _ = animation_tick.tick() => {
                // Check for suspend/resume via time jump
                let elapsed = loop_start.elapsed();
                if elapsed > SUSPEND_RESUME_THRESHOLD {
                    tracing::warn!("Time jump detected ({:?}), likely suspend/resume", elapsed);
                    handle.shutdown().await;
                    tokio::time::sleep(DBUS_CLEANUP_DELAY).await;
                    crate::remove_tray_lockfile();
                    return Ok(TrayExitReason::SuspendResume);
                }
                loop_start = Instant::now();

                // Check quit flag
                if should_quit.load(Ordering::SeqCst) {
                    break;
                }

                // Get animation metric and determine sleep state based on configured source
                let (animation_metric, is_sleeping) = match config.animation_source {
                    AnimationSource::CpuUsage => {
                        let metric = current_cpu;
                        (metric, metric < config.sleep_threshold_cpu)
                    }
                    AnimationSource::Frequency => {
                        let freq = CpuFrequency::read();
                        // For animation speed, use percentage (0-100)
                        let metric = freq.average_percentage();
                        // For sleep, compare average MHz against threshold in MHz
                        let avg_mhz = if freq.per_core.is_empty() {
                            0.0
                        } else {
                            freq.per_core.iter().sum::<u32>() as f32 / freq.per_core.len() as f32
                        };
                        (metric, avg_mhz < config.sleep_threshold_freq)
                    }
                    AnimationSource::Temperature => {
                        let temp = CpuTemperature::read();
                        let actual_temp = temp.max_temp();
                        // For animation speed, use percentage (0-100)
                        let metric = temp.percentage();
                        // For sleep, compare actual temp in °C against threshold
                        (metric, actual_temp < config.sleep_threshold_temp)
                    }
                };

                // Update animation frame if running
                if !is_sleeping {
                    let fps = config.calculate_fps(animation_metric);
                    if fps > 0.0 {
                        let frame_duration = Duration::from_secs_f32(1.0 / fps);
                        if last_frame_time.elapsed() >= frame_duration {
                            current_frame = (current_frame + 1) % RUN_FRAMES;
                            last_frame_time = Instant::now();

                            handle.update(|tray| {
                                tray.current_frame = current_frame;
                                tray.animation_metric = animation_metric;
                                tray.is_sleeping = is_sleeping;
                            }).await;
                        }
                    }
                } else {
                    // Update sleeping state even when not animating
                    handle.update(|tray| {
                        tray.animation_metric = animation_metric;
                        tray.is_sleeping = is_sleeping;
                    }).await;
                }
            }

            // Config check event (periodic polling as fallback to file watcher)
            _ = config_check.tick() => {
                let new_config = Config::load();
                let config_changed = new_config.show_percentage != config.show_percentage
                    || new_config.animation_source != config.animation_source
                    || (new_config.sleep_threshold_cpu - config.sleep_threshold_cpu).abs() > 0.1
                    || (new_config.sleep_threshold_freq - config.sleep_threshold_freq).abs() > 0.1
                    || (new_config.sleep_threshold_temp - config.sleep_threshold_temp).abs() > 0.1;

                // Check for theme file changes
                let new_mtime = get_theme_files_mtime();
                let theme_changed = new_mtime != tracked_theme_mtime;

                if config_changed || theme_changed {
                    if config_changed {
                        tracing::info!("Config changed: sleep_threshold={}, show_percentage={}",
                                      new_config.sleep_threshold, new_config.show_percentage);
                        config = new_config;
                    }
                    if theme_changed {
                        tracing::info!("Theme files changed, reloading colors");
                        tracked_theme_mtime = new_mtime;
                    }

                    // Get current theme color
                    let theme_color = get_theme_color();
                    let new_panel = is_panel_medium_or_larger();

                    handle.update(|tray| {
                        if config_changed {
                            tray.show_percentage = config.show_percentage;
                        }
                        if theme_changed {
                            tray.resources.update_colors(theme_color);
                        }
                        tray.panel_medium_or_larger = new_panel;
                    }).await;
                }
            }

            // Lockfile refresh event
            _ = lockfile_refresh.tick() => {
                crate::create_tray_lockfile();
            }
        }
    }

    // Shutdown sequence
    handle.shutdown().await;
    tokio::time::sleep(DBUS_CLEANUP_DELAY).await;
    crate::remove_tray_lockfile();

    Ok(TrayExitReason::Quit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recolor_image_preserves_alpha() {
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([100, 100, 100, 255])); // Opaque
        img.put_pixel(1, 0, image::Rgba([50, 50, 50, 128])); // Semi-transparent
        img.put_pixel(0, 1, image::Rgba([0, 0, 0, 0])); // Fully transparent
        img.put_pixel(1, 1, image::Rgba([200, 200, 200, 255])); // Opaque

        let recolored = recolor_image(&img, (255, 0, 0)); // Red

        // Opaque pixels should be red
        assert_eq!(recolored.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(recolored.get_pixel(1, 1), &image::Rgba([255, 0, 0, 255]));

        // Semi-transparent pixel should be red with preserved alpha
        assert_eq!(recolored.get_pixel(1, 0), &image::Rgba([255, 0, 0, 128]));

        // Fully transparent should stay transparent (color undefined but alpha=0)
        assert_eq!(recolored.get_pixel(0, 1)[3], 0);
    }

    #[test]
    fn test_composite_sprite_basic() {
        let mut target = RgbaImage::new(10, 10);
        let mut sprite = RgbaImage::new(3, 3);

        // Fill sprite with red
        for pixel in sprite.pixels_mut() {
            *pixel = image::Rgba([255, 0, 0, 255]);
        }

        composite_sprite(&mut target, &sprite, 2, 2);

        // Check sprite was composited at correct position
        assert_eq!(target.get_pixel(2, 2), &image::Rgba([255, 0, 0, 255]));
        assert_eq!(target.get_pixel(4, 4), &image::Rgba([255, 0, 0, 255]));

        // Check outside sprite area is still black/transparent
        assert_eq!(target.get_pixel(0, 0)[0], 0);
        assert_eq!(target.get_pixel(9, 9)[0], 0);
    }

    #[test]
    fn test_composite_sprite_skips_transparent() {
        let mut target = RgbaImage::new(10, 10);
        // Fill target with white
        for pixel in target.pixels_mut() {
            *pixel = image::Rgba([255, 255, 255, 255]);
        }

        let mut sprite = RgbaImage::new(2, 2);
        sprite.put_pixel(0, 0, image::Rgba([255, 0, 0, 255])); // Red opaque
        sprite.put_pixel(1, 0, image::Rgba([0, 255, 0, 0])); // Transparent

        composite_sprite(&mut target, &sprite, 0, 0);

        // Opaque pixel should overwrite
        assert_eq!(target.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]));

        // Transparent pixel should be skipped (target stays white)
        assert_eq!(target.get_pixel(1, 0), &image::Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn test_resources_update_colors_caches() {
        let mut resources = Resources::load().expect("Should load test resources");

        let color1 = (255, 0, 0); // Red
        let color2 = (0, 0, 255); // Blue

        // First update should recolor
        resources.update_colors(color1);
        assert_eq!(resources.last_theme_color, Some(color1));

        // Get first cat frame color
        let cat_frame = resources.get_cat_frame(0, false);
        // Find a non-transparent pixel and verify it's red-ish
        let has_red = cat_frame.pixels().any(|p| p[3] > 0 && p[0] > 200);
        assert!(has_red, "Should have recolored to red");

        // Second update with same color should be no-op (check by last_theme_color)
        resources.update_colors(color1);
        assert_eq!(resources.last_theme_color, Some(color1));

        // Update with different color should recolor
        resources.update_colors(color2);
        assert_eq!(resources.last_theme_color, Some(color2));

        let cat_frame = resources.get_cat_frame(0, false);
        let has_blue = cat_frame.pixels().any(|p| p[3] > 0 && p[2] > 200);
        assert!(has_blue, "Should have recolored to blue");
    }

    #[test]
    fn test_resources_get_cat_frame() {
        let resources = Resources::load().expect("Should load test resources");

        // Get sleeping frame
        let sleeping = resources.get_cat_frame(0, true);
        assert_eq!(sleeping.width(), CAT_SIZE);
        assert_eq!(sleeping.height(), CAT_SIZE);

        // Get running frame
        let running = resources.get_cat_frame(5, false);
        assert_eq!(running.width(), CAT_SIZE);
        assert_eq!(running.height(), CAT_SIZE);

        // Frame index should wrap
        let frame_high = resources.get_cat_frame(99, false);
        assert_eq!(frame_high.width(), CAT_SIZE);
    }

    #[test]
    fn test_resources_get_digit() {
        let resources = Resources::load().expect("Should load test resources");

        // All digits should be available
        for ch in "0123456789%".chars() {
            let digit = resources.get_digit(ch);
            assert!(digit.is_some(), "Digit '{}' should be loaded", ch);

            if let Some(img) = digit {
                assert_eq!(img.width(), DIGIT_WIDTH);
                assert_eq!(img.height(), DIGIT_HEIGHT);
            }
        }

        // Invalid character should return None
        assert!(resources.get_digit('X').is_none());
    }

    #[test]
    fn test_fallback_icon_creation() {
        let icon = create_fallback_icon(32, (200, 200, 200));
        assert_eq!(icon.width(), 32);
        assert_eq!(icon.height(), 32);

        // Center should be colored (within radius)
        let center_pixel = icon.get_pixel(16, 16);
        assert_eq!(center_pixel[0], 200);
        assert_eq!(center_pixel[1], 200);
        assert_eq!(center_pixel[2], 200);
        assert_eq!(center_pixel[3], 255); // Opaque

        // Corners should be transparent
        let corner_pixel = icon.get_pixel(0, 0);
        assert_eq!(corner_pixel[3], 0);
    }

    #[test]
    fn test_resources_load_or_fallback() {
        // Should always succeed (either loads or creates fallback)
        let resources = Resources::load_or_fallback();

        // Should have at least fallback resources
        assert!(!resources.cat_frames_original.is_empty());
        assert_eq!(resources.cat_frames_original.len(), RUN_FRAMES as usize);
    }
}
