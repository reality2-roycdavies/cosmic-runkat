//! COSMIC Panel Applet Module
//!
//! This is the main module that implements the animated running cat as a
//! native COSMIC desktop panel applet. The applet lives in the panel (taskbar)
//! and shows an animated cat sprite that runs faster when the system is busy.
//!
//! ## How it works
//!
//! 1. **Animation**: Cat sprites are embedded in the binary at compile time.
//!    A timer ticks ~30 times per second to advance the animation frame.
//!    The actual animation speed (FPS) is scaled by the chosen metric
//!    (CPU usage, frequency, or temperature).
//!
//! 2. **Theme integration**: The cat sprite is recolored to match the
//!    COSMIC desktop accent color. When the user changes their theme,
//!    the sprites are re-recolored from the original embedded PNGs.
//!
//! 3. **Popup**: Clicking the cat opens a popup showing per-core stats
//!    with colored progress bars. The popup is created via the COSMIC
//!    `app_popup` API which manages popup lifecycle and positioning.
//!
//! 4. **Settings**: A "Settings" button in the popup spawns a separate
//!    process (`cosmic-runkat --settings`) to avoid blocking the applet.

use cosmic::app::{Core, Task};
use cosmic::iced::window::Id;
use cosmic::iced::{Length, Rectangle};
use cosmic::iced_runtime::core::window;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, text};
use cosmic::Element;

use crate::config::{AnimationSource, Config};
use crate::constants::*;
use crate::cpu::{CpuMonitor, CpuUsage};
use crate::sysinfo::{CpuFrequency, CpuTemperature};
use crate::theme;

use image::RgbaImage;
use std::collections::VecDeque;
use std::time::Duration;

/// Application ID — must match the `.desktop` entry filename so the COSMIC
/// compositor can associate this process with its applet configuration.
const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat";

// ---------------------------------------------------------------------------
// Message enum — every possible event the applet can handle
// ---------------------------------------------------------------------------

/// Messages that drive the applet's state machine.
///
/// COSMIC/iced uses "The Elm Architecture" (TEA): the UI is a pure function
/// of state, and all state changes happen by sending `Message` values to the
/// `update()` method.
#[derive(Debug, Clone)]
pub enum Message {
    /// Fired ~30 times/sec by a timer subscription.  Advances the cat's
    /// animation frame based on the current metric.
    AnimationTick,

    /// Fired every 500ms.  Reads new CPU usage, frequency, and temperature
    /// data from the OS and updates the smoothed values.
    CpuUpdate,

    /// Fired every 500ms.  Reloads the config file and checks whether the
    /// desktop theme has changed (so we can recolor sprites).
    ConfigCheck,

    /// The compositor closed our popup window (e.g. user clicked elsewhere).
    PopupClosed(Id),

    /// Wraps a surface action (create popup / destroy popup) so the COSMIC
    /// runtime can process it.
    Surface(cosmic::surface::Action),

    /// User clicked the "Settings" button in the popup.
    OpenSettings,
}

// ---------------------------------------------------------------------------
// Sprite cache — manages the cat animation frames
// ---------------------------------------------------------------------------

/// Holds both the original (white) sprites and the theme-recolored versions.
///
/// We keep the originals around because when the user changes their desktop
/// theme, we need to recolor from the *original* white sprites — recoloring
/// already-colored sprites would lose detail.
struct SpriteCache {
    /// Original white sprites loaded once from embedded PNGs at startup.
    cat_frames_original: Vec<RgbaImage>,
    cat_sleep_original: RgbaImage,

    /// Theme-colored copies — these are what actually get rendered.
    cat_frames_colored: Vec<RgbaImage>,
    cat_sleep_colored: RgbaImage,

    /// The last color we applied.  If the theme color hasn't changed,
    /// we skip the (relatively expensive) recoloring step.
    last_theme_color: Option<(u8, u8, u8)>,
}

impl SpriteCache {
    /// Load all cat sprites from PNGs embedded in the binary via
    /// `include_bytes!`.  Each frame is stored twice — once as the
    /// original (for future recoloring) and once as the current colored
    /// version.
    fn load() -> Self {
        // Helper closure: try to decode a PNG byte slice into an RGBA image
        let load_img = |data: &[u8]| -> Option<RgbaImage> {
            image::load_from_memory(data).ok().map(|i| i.to_rgba8())
        };

        // Load the sleeping cat sprite (single frame)
        let cat_sleep = load_img(include_bytes!("../resources/cat-sleep.png"))
            .unwrap_or_else(|| create_fallback_icon(CAT_SIZE));

        // Load all 10 running animation frames
        let cat_frames: Vec<RgbaImage> = vec![
            load_img(include_bytes!("../resources/cat-run-0.png")),
            load_img(include_bytes!("../resources/cat-run-1.png")),
            load_img(include_bytes!("../resources/cat-run-2.png")),
            load_img(include_bytes!("../resources/cat-run-3.png")),
            load_img(include_bytes!("../resources/cat-run-4.png")),
            load_img(include_bytes!("../resources/cat-run-5.png")),
            load_img(include_bytes!("../resources/cat-run-6.png")),
            load_img(include_bytes!("../resources/cat-run-7.png")),
            load_img(include_bytes!("../resources/cat-run-8.png")),
            load_img(include_bytes!("../resources/cat-run-9.png")),
        ]
        .into_iter()
        // If any PNG fails to load, substitute a simple grey circle
        .map(|opt| opt.unwrap_or_else(|| create_fallback_icon(CAT_SIZE)))
        .collect();

        // Clone originals into the "colored" slots — they'll be recolored
        // immediately in init() when we know the theme accent color.
        Self {
            cat_frames_original: cat_frames.clone(),
            cat_sleep_original: cat_sleep.clone(),
            cat_frames_colored: cat_frames,
            cat_sleep_colored: cat_sleep,
            last_theme_color: None,
        }
    }

    /// Recolor all sprites to match the given RGB accent color.
    /// Does nothing if the color hasn't changed since last call.
    fn update_colors(&mut self, color: (u8, u8, u8)) {
        if self.last_theme_color == Some(color) {
            return; // color unchanged — skip expensive pixel work
        }

        // Recolor every running frame from the *original* white sprites
        self.cat_frames_colored = self
            .cat_frames_original
            .iter()
            .map(|img| recolor_image(img, color))
            .collect();

        self.cat_sleep_colored = recolor_image(&self.cat_sleep_original, color);
        self.last_theme_color = Some(color);
    }

    /// Return the appropriate frame as an iced image handle, ready to render.
    /// If `sleeping` is true, returns the sleep sprite instead of a run frame.
    fn frame_handle(&self, frame: u8, sleeping: bool) -> cosmic::iced::widget::image::Handle {
        let img = if sleeping {
            &self.cat_sleep_colored
        } else {
            // Wrap the frame index so it stays within bounds
            &self.cat_frames_colored[frame as usize % self.cat_frames_colored.len()]
        };
        rgba_to_handle(img)
    }
}

// ---------------------------------------------------------------------------
// Image utility functions
// ---------------------------------------------------------------------------

/// Replace every non-transparent pixel's color with `(r, g, b)`,
/// preserving the original alpha channel.  This is how we "tint"
/// the white cat sprites to match the user's accent color.
fn recolor_image(img: &RgbaImage, (r, g, b): (u8, u8, u8)) -> RgbaImage {
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        if pixel[3] > 0 {
            // pixel[3] is the alpha channel — 0 means fully transparent
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
    result
}

/// Convert an in-memory RGBA image into an iced `Handle` that the GUI
/// framework can display.
fn rgba_to_handle(img: &RgbaImage) -> cosmic::iced::widget::image::Handle {
    cosmic::iced::widget::image::Handle::from_rgba(
        img.width(),
        img.height(),
        img.as_raw().clone(),
    )
}

/// Create a simple grey circle as a fallback icon.  Used if any of the
/// embedded cat PNGs fail to decode (shouldn't happen, but better safe).
fn create_fallback_icon(size: u32) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let center = size as f32 / 2.0;
    let radius = size as f32 / 2.5;

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        // Simple distance check: if this pixel is inside the circle, color it
        if (dx * dx + dy * dy).sqrt() <= radius {
            *pixel = image::Rgba([200, 200, 200, 255]); // light grey, fully opaque
        }
    }
    img
}

// ---------------------------------------------------------------------------
// Main applet struct — holds all runtime state
// ---------------------------------------------------------------------------

/// The COSMIC panel applet.  This struct holds everything the applet needs:
/// animation state, system metrics, configuration, and popup state.
pub struct RunkatApplet {
    /// COSMIC framework core — provides access to applet helpers, window IDs, etc.
    core: Core,

    /// Pre-loaded and theme-colored cat sprites
    sprites: SpriteCache,

    // --- Animation state ---
    /// Which run-cycle frame we're currently showing (0..9)
    current_frame: u8,
    /// True when the monitored metric is below the sleep threshold
    is_sleeping: bool,
    /// When we last advanced the frame — used to control actual FPS
    last_frame_time: std::time::Instant,

    // --- System metrics ---
    /// Background thread that reads CPU usage via systemstat
    cpu_monitor: CpuMonitor,
    /// Latest CPU usage snapshot (aggregate + per-core percentages)
    cpu_usage: CpuUsage,
    /// Latest CPU frequency readings from sysfs
    cpu_frequency: CpuFrequency,
    /// Latest CPU temperature readings from hwmon
    cpu_temperature: CpuTemperature,

    // --- CPU smoothing ---
    /// Rolling window of recent CPU samples for averaging
    cpu_samples: VecDeque<f32>,
    /// The smoothed (averaged) CPU percentage shown in the UI
    smoothed_cpu: f32,

    // --- Configuration ---
    /// User preferences loaded from config.json
    config: Config,

    // --- Popup ---
    /// Window ID of the open popup, or None if closed
    popup: Option<Id>,

    // --- Theme ---
    /// Current accent color from the COSMIC theme (RGB)
    accent_color: (u8, u8, u8),
    /// Whether the current theme is dark (affects some color choices)
    theme_is_dark: bool,

    /// Pre-formatted tooltip text shown on hover
    tooltip: String,
}

// ---------------------------------------------------------------------------
// cosmic::Application trait implementation — the "Elm Architecture" core
// ---------------------------------------------------------------------------

impl cosmic::Application for RunkatApplet {
    /// Use a single-threaded executor since we do our heavy work on a
    /// background OS thread (CpuMonitor) rather than async tasks.
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Called once when the applet starts.  Sets up sprites, starts the
    /// CPU monitoring thread, reads initial sensor data, and loads config.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config = Config::load();
        let mut sprites = SpriteCache::load();

        // Read the COSMIC theme's accent color and recolor sprites to match
        let theme_colors = theme::get_cosmic_theme_colors();
        sprites.update_colors(theme_colors.foreground);

        // Start the background CPU monitoring thread
        let cpu_monitor = CpuMonitor::new();
        cpu_monitor.start(CPU_SAMPLE_INTERVAL);

        let applet = Self {
            core,
            sprites,
            current_frame: 0,
            is_sleeping: true, // start sleeping until we get real data
            last_frame_time: std::time::Instant::now(),
            cpu_monitor,
            cpu_usage: CpuUsage::default(),
            cpu_frequency: CpuFrequency::read(),
            cpu_temperature: CpuTemperature::read(),
            cpu_samples: VecDeque::with_capacity(CPU_SAMPLE_COUNT),
            smoothed_cpu: 0.0,
            config,
            popup: None,
            accent_color: theme_colors.foreground,
            theme_is_dark: theme_colors.is_dark,
            tooltip: String::from("RunKat"),
        };

        // Task::none() means no asynchronous work to do at startup
        (applet, Task::none())
    }

    /// Called by the compositor when a window (our popup) is closed.
    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The heart of the Elm Architecture: handle a message and update state.
    /// Returns a `Task` if there's asynchronous follow-up work (usually none).
    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            // ---------------------------------------------------------------
            // ANIMATION TICK (~30 times per second)
            // ---------------------------------------------------------------
            // 1. Read the relevant metric based on user's chosen source
            // 2. Determine if the cat should be sleeping (metric below threshold)
            // 3. If awake, advance the animation frame at the calculated FPS
            Message::AnimationTick => {
                let (metric, sleeping) = match self.config.animation_source {
                    AnimationSource::CpuUsage => {
                        let m = self.smoothed_cpu;
                        // Sleep when CPU usage is below the configured threshold
                        (m, m < self.config.sleep_threshold_cpu)
                    }
                    AnimationSource::Frequency => {
                        // Use percentage for animation speed, but compare
                        // absolute MHz against threshold for sleep decision
                        let metric = self.cpu_frequency.average_percentage();
                        let avg_mhz = self.cpu_frequency.average_mhz() as f32;
                        (metric, avg_mhz < self.config.sleep_threshold_freq)
                    }
                    AnimationSource::Temperature => {
                        // Use percentage of critical temp for animation speed,
                        // but compare actual degrees for sleep decision
                        let actual = self.cpu_temperature.max_temp();
                        let metric = self.cpu_temperature.percentage();
                        (metric, actual < self.config.sleep_threshold_temp)
                    }
                };

                self.is_sleeping = sleeping;

                // Only advance animation frames when the cat is awake
                if !sleeping {
                    let fps = self.config.calculate_fps(metric);
                    if fps > 0.0 {
                        // Calculate how long each frame should be shown
                        let frame_duration = Duration::from_secs_f32(1.0 / fps);
                        if self.last_frame_time.elapsed() >= frame_duration {
                            // Wrap around: frame 0, 1, 2, ..., 9, 0, 1, ...
                            self.current_frame = (self.current_frame + 1) % RUN_FRAMES;
                            self.last_frame_time = std::time::Instant::now();
                        }
                    }
                }
            }

            // ---------------------------------------------------------------
            // CPU DATA UPDATE (every 500ms)
            // ---------------------------------------------------------------
            Message::CpuUpdate => {
                // Get the latest snapshot from the background monitoring thread
                self.cpu_usage = self.cpu_monitor.current();

                // Smooth the aggregate CPU percentage using a rolling average.
                // This prevents the animation from jittering on brief spikes.
                let raw = self.cpu_usage.aggregate;
                self.cpu_samples.push_back(raw);
                if self.cpu_samples.len() > CPU_SAMPLE_COUNT {
                    self.cpu_samples.pop_front(); // remove oldest sample
                }
                self.smoothed_cpu = if self.cpu_samples.is_empty() {
                    raw
                } else {
                    self.cpu_samples.iter().sum::<f32>() / self.cpu_samples.len() as f32
                };

                // Also refresh frequency and temperature (read directly from sysfs)
                self.cpu_frequency = CpuFrequency::read();
                self.cpu_temperature = CpuTemperature::read();

                // Update the hover tooltip text
                self.tooltip = self.make_tooltip();
            }

            // ---------------------------------------------------------------
            // CONFIG / THEME CHECK (every 500ms)
            // ---------------------------------------------------------------
            Message::ConfigCheck => {
                // Reload config from disk so settings changes take effect
                // without restarting the applet
                self.config = Config::load();

                // Check if the desktop theme accent color has changed
                let theme_colors = theme::get_cosmic_theme_colors();
                if theme_colors.foreground != self.accent_color
                    || theme_colors.is_dark != self.theme_is_dark
                {
                    self.accent_color = theme_colors.foreground;
                    self.theme_is_dark = theme_colors.is_dark;
                    // Re-tint all sprites to match the new accent color
                    self.sprites.update_colors(theme_colors.foreground);
                }
            }

            // ---------------------------------------------------------------
            // POPUP LIFECYCLE
            // ---------------------------------------------------------------
            Message::PopupClosed(id) => {
                // Only clear our popup state if the closed window was our popup
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }

            Message::Surface(action) => {
                // Forward surface actions (popup create/destroy) to the COSMIC runtime
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }

            // ---------------------------------------------------------------
            // SETTINGS
            // ---------------------------------------------------------------
            // We spawn a *separate OS process* for the settings window.
            // This is done on a background thread because std::process::Command
            // can briefly block while setting up the child process.
            Message::OpenSettings => {
                std::thread::spawn(|| {
                    // Don't spawn a second instance if already running
                    if let Ok(output) = std::process::Command::new("pgrep").arg("-f").arg("cosmic-applet-settings").output() {
                        if output.status.success() { return; }
                    }
                    let result = std::process::Command::new("cosmic-applet-settings")
                        .arg("runkat")
                        .spawn();
                    if result.is_err() {
                        let exe = std::env::current_exe().unwrap_or_default();
                        let _ = std::process::Command::new(exe).arg("--settings").spawn();
                    }
                });
            }
        }

        Task::none()
    }

    /// Register timer subscriptions.  These fire periodically and generate
    /// the messages that drive the applet's update loop.
    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::Subscription::batch([
            // ~30 FPS animation tick
            cosmic::iced::time::every(Duration::from_millis(33))
                .map(|_| Message::AnimationTick),
            // CPU / sensor data refresh
            cosmic::iced::time::every(Duration::from_millis(500))
                .map(|_| Message::CpuUpdate),
            // Config & theme change detection
            cosmic::iced::time::every(Duration::from_millis(500))
                .map(|_| Message::ConfigCheck),
        ])
    }

    /// Build the panel button widget.  This is what the user sees in the
    /// COSMIC panel — a small animated cat image with an optional CPU% label.
    fn view(&self) -> Element<'_, Message> {
        // Get the current cat frame as a renderable image handle
        let handle = self.sprites.frame_handle(self.current_frame, self.is_sleeping);

        // Ask the COSMIC applet framework for the suggested icon size
        let suggested = self.core.applet.suggested_size(true);
        let img_size = suggested.0 as f32;

        let cat_image = cosmic::iced::widget::image(handle)
            .width(Length::Fixed(img_size))
            .height(Length::Fixed(img_size));

        // Optionally show a "42%" label next to the cat (CPU mode only)
        let content: Element<Message> = if self.config.show_percentage
            && self.config.animation_source == AnimationSource::CpuUsage
            && !self.is_sleeping
        {
            let pct_text = text::body(format!("{:.0}%", self.smoothed_cpu));
            cosmic::iced::widget::row![cat_image, pct_text]
                .spacing(4)
                .align_y(cosmic::iced::Alignment::Center)
                .into()
        } else {
            cat_image.into()
        };

        // Wrap in an applet button that toggles the popup on click
        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .button_from_element(content, true)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    // Popup is open — close it
                    Message::Surface(destroy_popup(id))
                } else {
                    // Popup is closed — create it using the COSMIC popup API.
                    // `app_popup` takes a closure that runs with mutable access
                    // to our applet state (to store the new popup ID).
                    Message::Surface(app_popup::<RunkatApplet>(
                        move |state: &mut RunkatApplet| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);

                            // Size the popup to fit the number of CPU cores
                            let core_count = state.cpu_usage.per_core.len().max(
                                state.cpu_frequency.per_core.len().max(
                                    state.cpu_temperature.per_core.len(),
                                ),
                            );
                            let visible_rows =
                                (core_count as u32 + 1).min(POPUP_MAX_ROWS);
                            let popup_height =
                                POPUP_BASE_HEIGHT + visible_rows * POPUP_ROW_HEIGHT;

                            // Get the main window ID so the popup can anchor to it.
                            // If unavailable (shouldn't happen), fall back gracefully.
                            let Some(main_id) = state.core.main_window_id() else {
                                tracing::error!("No main window ID for popup");
                                return state.core.applet.get_popup_settings(
                                    Id::unique(),
                                    new_id,
                                    Some((POPUP_WIDTH, popup_height)),
                                    None,
                                    None,
                                );
                            };

                            // Configure popup position relative to the panel button
                            let mut popup_settings = state.core.applet.get_popup_settings(
                                main_id,
                                new_id,
                                Some((POPUP_WIDTH, popup_height)),
                                None,
                                None,
                            );
                            popup_settings.positioner.anchor_rect = Rectangle {
                                x: (bounds.x - offset.x) as i32,
                                y: (bounds.y - offset.y) as i32,
                                width: bounds.width as i32,
                                height: bounds.height as i32,
                            };
                            popup_settings
                        },
                        // This closure renders the popup's content each frame
                        Some(Box::new(|state: &RunkatApplet| {
                            Element::from(state.core.applet.popup_container(
                                state.popup_content(),
                            ))
                            .map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        // Wrap everything in a tooltip (shown on hover when popup is closed)
        Element::from(self.core.applet.applet_tooltip::<Message>(
            btn,
            self.tooltip.clone(),
            self.popup.is_some(),
            Message::Surface,
            None,
        ))
    }

    /// This method is required by the COSMIC Application trait but our popup
    /// content is rendered via the closure passed to `app_popup()` in `view()`,
    /// so this just returns empty content.
    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        "".into()
    }

    /// Apply the standard COSMIC applet appearance (transparent background, etc.)
    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

// ---------------------------------------------------------------------------
// Helper methods on RunkatApplet
// ---------------------------------------------------------------------------

impl RunkatApplet {
    /// Generate a short tooltip string like "CPU: 42%" or "CPU: 3200 MHz"
    fn make_tooltip(&self) -> String {
        match self.config.animation_source {
            AnimationSource::CpuUsage => format!("CPU: {:.0}%", self.smoothed_cpu),
            AnimationSource::Frequency => {
                format!("CPU: {} MHz", self.cpu_frequency.average_mhz())
            }
            AnimationSource::Temperature => {
                format!("CPU: {:.1}\u{00b0}C", self.cpu_temperature.max_temp())
            }
        }
    }

    /// Build the popup content: title, scrollable stats area, status text,
    /// and a "Settings" button.  The stats shown depend on the configured
    /// animation source (CPU usage / frequency / temperature).
    fn popup_content(&self) -> widget::Column<'_, Message> {
        use cosmic::iced::widget::{column, container, horizontal_space, row, scrollable, Space};
        use cosmic::iced::{Alignment, Color};

        // Title changes based on what we're monitoring
        let title_text = match self.config.animation_source {
            AnimationSource::CpuUsage => "CPU Usage",
            AnimationSource::Frequency => "CPU Frequency",
            AnimationSource::Temperature => "CPU Temperature",
        };

        let title_row = row![
            text::body(title_text),
            horizontal_space(), // push title to the left
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Build the per-core statistics area (different layout per source)
        let stats_content: Element<'_, Message> = match self.config.animation_source {
            AnimationSource::CpuUsage => {
                // Show total CPU usage + one row per core
                let overall = self.cpu_usage.aggregate;
                let overall_row = row![
                    text::caption("Total:").width(Length::Fixed(80.0)),
                    self.progress_bar(overall, 100.0, false),
                    text::caption(format!("{:5.1}%", overall)).width(Length::Fixed(55.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut core_column = column![].spacing(2);
                for (i, &pct) in self.cpu_usage.per_core.iter().enumerate() {
                    let core_row = row![
                        text::caption(format!("CPU{}:", i)).width(Length::Fixed(80.0)),
                        self.progress_bar(pct, 100.0, false),
                        text::caption(format!("{:5.1}%", pct)).width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    core_column = core_column.push(core_row);
                }

                column![overall_row, core_column].spacing(4).into()
            }

            AnimationSource::Frequency => {
                // Show average frequency + one row per core
                let avg_mhz = self.cpu_frequency.average_mhz();
                let max_mhz = self.cpu_frequency.max_per_core.first().copied().unwrap_or(1);

                let avg_row = row![
                    text::caption("Avg:").width(Length::Fixed(80.0)),
                    self.progress_bar(avg_mhz as f32, max_mhz as f32, true),
                    text::caption(format!("{} MHz", avg_mhz)).width(Length::Fixed(80.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut core_column = column![].spacing(2);
                for (i, &mhz) in self.cpu_frequency.per_core.iter().enumerate() {
                    let max = self.cpu_frequency.max_per_core.get(i).copied().unwrap_or(1);
                    let core_row = row![
                        text::caption(format!("CPU{}:", i)).width(Length::Fixed(80.0)),
                        self.progress_bar(mhz as f32, max as f32, true),
                        text::caption(format!("{} MHz", mhz)).width(Length::Fixed(80.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    core_column = core_column.push(core_row);
                }

                column![avg_row, core_column].spacing(4).into()
            }

            AnimationSource::Temperature => {
                // Show max temp, package temp (if available), + per-core temps
                let max_temp = self.cpu_temperature.max_temp();
                let critical = self.cpu_temperature.critical.unwrap_or(100.0);

                let max_row = row![
                    text::caption("Max:").width(Length::Fixed(80.0)),
                    self.progress_bar(max_temp, critical, false),
                    text::caption(format!("{:.1}\u{00b0}C", max_temp)).width(Length::Fixed(55.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut temp_column = column![max_row].spacing(2);

                // Package temp is the overall CPU die temperature (not all
                // systems expose this)
                if let Some(pkg_temp) = self.cpu_temperature.package {
                    let pkg_row = row![
                        text::caption("Package:").width(Length::Fixed(80.0)),
                        self.progress_bar(pkg_temp, critical, false),
                        text::caption(format!("{:.1}\u{00b0}C", pkg_temp))
                            .width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    temp_column = temp_column.push(pkg_row);
                }

                for (i, &temp) in self.cpu_temperature.per_core.iter().enumerate() {
                    let core_row = row![
                        text::caption(format!("Core {}:", i)).width(Length::Fixed(80.0)),
                        self.progress_bar(temp, critical, false),
                        text::caption(format!("{:.1}\u{00b0}C", temp))
                            .width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    temp_column = temp_column.push(core_row);
                }

                temp_column.into()
            }
        };

        // Fun status text describing the cat's current state
        let status_text = match self.config.animation_source {
            AnimationSource::CpuUsage => {
                if self.cpu_usage.aggregate < self.config.sleep_threshold_cpu {
                    "Cat is sleeping..."
                } else {
                    "Cat is running!"
                }
            }
            AnimationSource::Frequency => {
                let avg_mhz = self.cpu_frequency.average_mhz() as f32;
                if avg_mhz < self.config.sleep_threshold_freq {
                    "Cat is idle..."
                } else {
                    "Cat is boosting!"
                }
            }
            AnimationSource::Temperature => {
                let max_temp = self.cpu_temperature.max_temp();
                if max_temp < self.config.sleep_threshold_temp {
                    "Cat is cool..."
                } else if max_temp > TEMP_HOT_THRESHOLD {
                    "Cat is HOT!"
                } else {
                    "Cat is warm..."
                }
            }
        };

        let bottom_row = row![
            text::caption(status_text),
            horizontal_space(),
            widget::button::standard("Settings").on_press(Message::OpenSettings),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Make the stats area scrollable for systems with many cores
        let scrollable_stats = scrollable(stats_content)
            .height(Length::Fixed(POPUP_MAX_SCROLL_HEIGHT));

        // Thin horizontal line divider between popup sections
        let divider = || {
            container(Space::new(Length::Fill, Length::Fixed(1.0))).style(
                |theme: &cosmic::Theme| {
                    let cosmic = theme.cosmic();
                    container::Style {
                        background: Some(cosmic::iced::Background::Color(
                            Color::from(cosmic.palette.neutral_5),
                        )),
                        ..Default::default()
                    }
                },
            )
        };

        // Assemble: title | divider | stats | divider | status + settings button
        column![title_row, divider(), scrollable_stats, divider(), bottom_row,]
            .spacing(8)
            .padding(12)
    }

    /// Create a colored progress bar widget.
    ///
    /// The bar is built from two nested containers:
    /// - Outer: full-width background (semi-transparent grey)
    /// - Inner: filled portion whose width = `value / max * BAR_WIDTH`
    ///
    /// ## Color scheme
    ///
    /// For CPU usage and temperature bars (`is_freq = false`):
    /// - >90% fill: **red** — high load or dangerously hot
    /// - >70% fill: **orange** — elevated, worth noting
    /// - >50% fill: **yellow** — moderate activity
    /// - <=50% fill: **accent color** — normal, blends with theme
    ///
    /// For frequency bars (`is_freq = true`):
    /// - Uses a **blue gradient** that gets brighter at higher frequencies
    fn progress_bar(&self, value: f32, max: f32, is_freq: bool) -> Element<'_, Message> {
        use cosmic::iced::widget::{container, Space};
        use cosmic::iced::Color;

        // Calculate what fraction of the bar should be filled (0.0 to 1.0)
        let pct = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
        let filled_width = pct * BAR_WIDTH;

        // Choose bar color based on the fill percentage and bar type
        let bar_color = if is_freq {
            // Blue gradient: darker blue at low freq, brighter at high freq
            Color::from_rgb8(
                (50.0 + pct * 150.0) as u8,
                (100.0 + pct * 100.0) as u8,
                220,
            )
        } else if pct > 0.9 {
            Color::from_rgb8(220, 50, 50)   // red — danger zone
        } else if pct > 0.7 {
            Color::from_rgb8(220, 150, 50)  // orange — elevated
        } else if pct > 0.5 {
            Color::from_rgb8(200, 200, 50)  // yellow — moderate
        } else {
            // Use the desktop accent color for normal values
            let (r, g, b) = self.accent_color;
            Color::from_rgb8(r, g, b)
        };

        // Inner container: the colored "filled" part of the bar
        let inner =
            container(Space::new(Length::Fixed(filled_width), Length::Fixed(BAR_HEIGHT - 2.0)))
                .style(move |_: &cosmic::Theme| container::Style {
                    background: Some(cosmic::iced::Background::Color(bar_color)),
                    border: cosmic::iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });

        // Outer container: the grey "track" background
        container(inner)
            .width(Length::Fixed(BAR_WIDTH))
            .height(Length::Fixed(BAR_HEIGHT))
            .style(|_: &cosmic::Theme| container::Style {
                background: Some(cosmic::iced::Background::Color(Color::from_rgba(
                    0.5, 0.5, 0.5, 0.2,
                ))),
                border: cosmic::iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}

/// Entry point: launch the COSMIC panel applet.
pub fn run_applet() -> cosmic::iced::Result {
    cosmic::applet::run::<RunkatApplet>(())
}
