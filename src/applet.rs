//! COSMIC Panel Applet Module
//!
//! This is the main module that implements the animated running cat as a
//! native COSMIC desktop panel applet. The applet lives in the panel (taskbar)
//! and shows an animated cat sprite that runs faster when the system is busy.
//!
//! ## How it works
//!
//! 1. **Animation**: Runner sprites (cat, horse, parrot, from RunCat365) are
//!    embedded in the binary at compile time.  A timer ticks about 30 times
//!    per second to advance the animation frame.  The animation speed (FPS)
//!    is scaled by the chosen metric (CPU usage, frequency, temperature,
//!    memory or GPU).  While asleep, little z's float up from the runner.
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
use cosmic::iced::window;
use cosmic::surface::action::{app_popup, destroy_popup};
use cosmic::widget::{self, text};
use cosmic::Element;

use crate::config::{AnimationSource, Config, Runner};
use crate::constants::*;
use crate::cpu::{CpuMonitor, CpuUsage};
use crate::gpu::{GpuInfo, GpuMonitor};
use crate::sysinfo::{kib_to_gib, CpuFrequency, CpuTemperature, MemoryInfo};
use crate::theme;

use image::RgbaImage;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

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
// Runner artwork
// ---------------------------------------------------------------------------

/// Embedded artwork for one runner, plus where its sleep z's float.
struct RunnerArt {
    run: &'static [&'static [u8]],
    sleep: &'static [u8],
    /// Start and end of the z's path (top-left of each "z"), in pixels of
    /// the cropped 32x24 sprite.  Starts just above the sleeping head.
    zzz_from: (f32, f32),
    zzz_to: (f32, f32),
}

fn runner_art(runner: Runner) -> RunnerArt {
    match runner {
        // Run frames: RunCat365 (Apache-2.0).  Sleep sprite: tools/draw_sleep_sprites.py
        Runner::Cat => RunnerArt {
            run: &[
                include_bytes!("../resources/cat-run-0.png"),
                include_bytes!("../resources/cat-run-1.png"),
                include_bytes!("../resources/cat-run-2.png"),
                include_bytes!("../resources/cat-run-3.png"),
                include_bytes!("../resources/cat-run-4.png"),
            ],
            sleep: include_bytes!("../resources/cat-sleep.png"),
            zzz_from: (15.5, 7.0),
            zzz_to: (9.0, -0.5),
        },
        // Run frames: RunCat365 (Apache-2.0).  Sleep sprite: tools/draw_sleep_sprites.py
        Runner::Horse => RunnerArt {
            run: &[
                include_bytes!("../resources/horse-run-0.png"),
                include_bytes!("../resources/horse-run-1.png"),
                include_bytes!("../resources/horse-run-2.png"),
                include_bytes!("../resources/horse-run-3.png"),
                include_bytes!("../resources/horse-run-4.png"),
            ],
            sleep: include_bytes!("../resources/horse-sleep.png"),
            zzz_from: (13.0, 5.5),
            zzz_to: (6.5, -0.5),
        },
        // RunCat365 (Apache-2.0).  Sleeps in the head-down frame of its bob.
        Runner::Parrot => RunnerArt {
            run: &[
                include_bytes!("../resources/parrot-run-0.png"),
                include_bytes!("../resources/parrot-run-1.png"),
                include_bytes!("../resources/parrot-run-2.png"),
                include_bytes!("../resources/parrot-run-3.png"),
                include_bytes!("../resources/parrot-run-4.png"),
                include_bytes!("../resources/parrot-run-5.png"),
                include_bytes!("../resources/parrot-run-6.png"),
                include_bytes!("../resources/parrot-run-7.png"),
                include_bytes!("../resources/parrot-run-8.png"),
                include_bytes!("../resources/parrot-run-9.png"),
            ],
            sleep: include_bytes!("../resources/parrot-run-6.png"),
            zzz_from: (7.5, 5.0),
            zzz_to: (1.5, -0.5),
        },
    }
}

// ---------------------------------------------------------------------------
// Sprite cache — the current runner's frames, tinted and ready to draw
// ---------------------------------------------------------------------------

type Handle = cosmic::iced::widget::image::Handle;

/// Tinted, ready-to-render frames for the selected runner.  Rebuilt from
/// the embedded PNGs whenever the runner or the theme color changes.
struct SpriteCache {
    runner: Runner,
    color: (u8, u8, u8),
    art: RunnerArt,
    run: Vec<Handle>,
    /// Tinted sleep pose without z's
    sleep_base: RgbaImage,
    /// Sleep pose with the z's at their latest position
    sleep: Handle,
}

impl SpriteCache {
    fn new(runner: Runner, color: (u8, u8, u8)) -> Self {
        let art = runner_art(runner);
        let load = |data: &[u8]| -> RgbaImage {
            let img = image::load_from_memory(data)
                .map(|i| i.to_rgba8())
                // If a PNG fails to decode, substitute a simple grey circle
                .unwrap_or_else(|_| create_fallback_icon(CAT_SIZE));
            tint(&crop_sprite(&img), color)
        };
        let run = art.run.iter().map(|d| rgba_to_handle(&load(d))).collect();
        let sleep_base = load(art.sleep);
        let sleep = rgba_to_handle(&sleep_base);
        Self { runner, color, art, run, sleep_base, sleep }
    }

    /// Rebuild if the runner or color changed.  Returns true if it did.
    fn ensure(&mut self, runner: Runner, color: (u8, u8, u8)) -> bool {
        if self.runner == runner && self.color == color {
            return false;
        }
        *self = Self::new(runner, color);
        true
    }

    fn frame_count(&self) -> usize {
        self.run.len()
    }

    fn run_frame(&self, frame: usize) -> Handle {
        self.run[frame % self.run.len()].clone()
    }

    /// Redraw the sleep pose with the z's at time `t` seconds into the nap.
    fn animate_sleep(&mut self, t: f32) {
        let mut img = self.sleep_base.clone();
        draw_zzz(&mut img, &self.art, t, self.color);
        self.sleep = rgba_to_handle(&img);
    }
}

// ---------------------------------------------------------------------------
// Image utility functions
// ---------------------------------------------------------------------------

/// Cut a 32x32 sprite down to the strip that actually holds the runner.
fn crop_sprite(img: &RgbaImage) -> RgbaImage {
    let top = CAT_CROP_TOP.min(img.height());
    let height = CAT_CROP_HEIGHT.min(img.height() - top);
    image::imageops::crop_imm(img, 0, top, img.width(), height).to_image()
}

/// Paint a sprite in the theme color.  Dark pixels become solid color and
/// light pixels become see-through, so a black silhouette keeps its soft
/// edges and the parrot's white eye and beak still show up as details.
fn tint(img: &RgbaImage, (r, g, b): (u8, u8, u8)) -> RgbaImage {
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        let [pr, pg, pb, pa] = pixel.0;
        let lum = (0.299 * pr as f32 + 0.587 * pg as f32 + 0.114 * pb as f32) / 255.0;
        let alpha = (pa as f32 * (1.0 - lum)).round() as u8;
        *pixel = image::Rgba([r, g, b, alpha]);
    }
    result
}

/// Draw the floating z's onto a sleeping sprite.  Three z's are spaced
/// evenly through the cycle; each rises from `zzz_from` to `zzz_to`,
/// growing a little, fading in at the start and out near the top.
fn draw_zzz(img: &mut RgbaImage, art: &RunnerArt, t: f32, color: (u8, u8, u8)) {
    for k in 0..3 {
        let phase = (t / ZZZ_PERIOD_SECS + k as f32 / 3.0).fract();
        let x = art.zzz_from.0 + (art.zzz_to.0 - art.zzz_from.0) * phase;
        let y = art.zzz_from.1 + (art.zzz_to.1 - art.zzz_from.1) * phase;
        let size = 2.0 + 1.4 * phase;
        let opacity = (phase / 0.15).min(1.0) * ((1.0 - phase) / 0.4).min(1.0);
        let (x1, y1) = (x + size, y + size);
        for (a, b) in [((x, y), (x1, y)), ((x1, y), (x, y1)), ((x, y1), (x1, y1))] {
            stamp_segment(img, a, b, 0.9, opacity, color);
        }
    }
}

/// Draw an anti-aliased line segment of the given width, blending over the
/// existing pixels.
fn stamp_segment(
    img: &mut RgbaImage,
    (ax, ay): (f32, f32),
    (bx, by): (f32, f32),
    width: f32,
    opacity: f32,
    (r, g, b): (u8, u8, u8),
) {
    if opacity <= 0.0 {
        return;
    }
    let pad = width / 2.0 + 1.0;
    let x0 = (ax.min(bx) - pad).floor().max(0.0) as u32;
    let y0 = (ay.min(by) - pad).floor().max(0.0) as u32;
    let x1 = ((ax.max(bx) + pad).ceil().max(0.0) as u32).min(img.width());
    let y1 = ((ay.max(by) + pad).ceil().max(0.0) as u32).min(img.height());
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = (dx * dx + dy * dy).max(f32::EPSILON);
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            let t = (((cx - ax) * dx + (cy - ay) * dy) / len2).clamp(0.0, 1.0);
            let (qx, qy) = (ax + t * dx - cx, ay + t * dy - cy);
            let dist = (qx * qx + qy * qy).sqrt();
            let cover = (width / 2.0 + 0.5 - dist).clamp(0.0, 1.0) * opacity;
            if cover <= 0.0 {
                continue;
            }
            let pixel = img.get_pixel_mut(px, py);
            let dst = pixel.0[3] as f32 / 255.0;
            let out = cover + dst * (1.0 - cover);
            *pixel = image::Rgba([r, g, b, (out * 255.0).round() as u8]);
        }
    }
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

    /// The selected runner's frames, tinted to the theme
    sprites: SpriteCache,

    // --- Animation state ---
    /// Which run-cycle frame we're currently showing
    current_frame: usize,
    /// True when the monitored metric is below the sleep threshold
    is_sleeping: bool,
    /// When we last advanced the frame — used to control actual FPS
    last_frame_time: Instant,
    /// When the current nap started — drives the floating z's
    sleep_started: Instant,

    // --- System metrics ---
    /// Background thread that reads CPU usage via systemstat
    cpu_monitor: CpuMonitor,
    /// Latest CPU usage snapshot (aggregate + per-core percentages)
    cpu_usage: CpuUsage,
    /// Latest CPU frequency readings from sysfs
    cpu_frequency: CpuFrequency,
    /// Latest CPU temperature readings from hwmon
    cpu_temperature: CpuTemperature,
    /// Latest RAM / swap usage
    memory: MemoryInfo,
    /// Background thread polling GPU load (only while GPU is the source)
    gpu_monitor: GpuMonitor,
    /// Latest GPU readings
    gpu_info: GpuInfo,
    /// Smoothed load of the busiest GPU
    smoothed_gpu: f32,

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

        // Read the COSMIC theme's accent color and tint the sprites to match
        let theme_colors = theme::get_cosmic_theme_colors();
        let sprites = SpriteCache::new(config.runner, theme_colors.foreground);

        // Start the background CPU monitoring thread
        let cpu_monitor = CpuMonitor::new();
        cpu_monitor.start(CPU_SAMPLE_INTERVAL);

        let applet = Self {
            core,
            sprites,
            current_frame: 0,
            is_sleeping: true, // start sleeping until we get real data
            last_frame_time: Instant::now(),
            sleep_started: Instant::now(),
            cpu_monitor,
            cpu_usage: CpuUsage::default(),
            cpu_frequency: CpuFrequency::read(),
            cpu_temperature: CpuTemperature::read(),
            memory: MemoryInfo::read(),
            gpu_monitor: GpuMonitor::start(),
            gpu_info: GpuInfo::default(),
            smoothed_gpu: 0.0,
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
                let (metric, sleeping) = self.current_metric();

                if sleeping && !self.is_sleeping {
                    self.sleep_started = Instant::now();
                }
                self.is_sleeping = sleeping;

                if sleeping {
                    // Only the z's move while asleep
                    self.sprites
                        .animate_sleep(self.sleep_started.elapsed().as_secs_f32());
                } else {
                    let fps = self.config.calculate_fps(metric).max(0.1);
                    let frame = Duration::from_secs_f32(1.0 / fps);
                    let elapsed = self.last_frame_time.elapsed();
                    if elapsed >= frame {
                        // At high FPS more than one frame can pass per tick
                        let steps = (elapsed.as_secs_f32() / frame.as_secs_f32()) as u32;
                        self.current_frame = (self.current_frame + steps as usize)
                            % self.sprites.frame_count();
                        self.last_frame_time += frame * steps;
                        // After a pause (e.g. waking up), don't try to catch up
                        if self.last_frame_time.elapsed() > frame {
                            self.last_frame_time = Instant::now();
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
                self.memory = MemoryInfo::read();

                // GPU is only polled while it is the selected source
                let gpu_active = self.config.animation_source == AnimationSource::Gpu;
                self.gpu_monitor.set_active(gpu_active);
                if gpu_active {
                    self.gpu_info = self.gpu_monitor.current();
                    let busy = self.gpu_info.max_busy();
                    self.smoothed_gpu += (busy - self.smoothed_gpu) * 0.4;
                }

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
                }

                // Re-tint on theme change, reload on runner change
                if self.sprites.ensure(self.config.runner, self.accent_color) {
                    self.current_frame %= self.sprites.frame_count();
                    self.sprites
                        .animate_sleep(self.sleep_started.elapsed().as_secs_f32());
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
                    // Try unified settings hub first, fall back to standalone
                    let unified = std::process::Command::new("cosmic-applet-settings")
                        .arg(APP_ID)
                        .spawn();
                    if unified.is_err() {
                        let exe = std::env::current_exe()
                            .unwrap_or_else(|_| "cosmic-runkat".into());
                        if let Err(e) = std::process::Command::new(exe)
                            .arg("--settings-standalone")
                            .spawn()
                        {
                            eprintln!("Failed to launch settings: {e}");
                        }
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
            // Animation tick: fast while running, slow while only z's move
            cosmic::iced::time::every(if self.is_sleeping { SLEEP_TICK } else { RUN_TICK })
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
        // Current frame as a renderable image handle
        let handle = if self.is_sleeping {
            self.sprites.sleep.clone()
        } else {
            self.sprites.run_frame(self.current_frame)
        };

        // Size the cat from the panel's icon size.  The sprites are cropped
        // to 32x24, so at icon height the cat is drawn larger than it would
        // be inside a square icon.
        let (icon, _) = self.core.applet.suggested_size(true);
        let (pad_major, _) = self.core.applet.suggested_padding(true);
        let horizontal = self.core.applet.is_horizontal();
        let aspect = CAT_SIZE as f32 / CAT_CROP_HEIGHT as f32;
        let (cat_w, cat_h) = if horizontal {
            let h = icon as f32;
            (h * aspect, h)
        } else {
            let w = icon as f32;
            (w, w / aspect)
        };

        let cat_image = cosmic::iced::widget::image(handle)
            .width(Length::Fixed(cat_w))
            .height(Length::Fixed(cat_h));

        // Optionally show a "42%" label next to the runner (% sources only)
        let show_pct = self.config.show_percentage
            && self.config.animation_source.is_percentage()
            && !self.is_sleeping;
        let content: Element<Message> = if show_pct {
            let pct_num = cosmic::iced::widget::text(format!("{:.0}", self.current_metric().0))
                .font(cosmic::iced::Font::MONOSPACE)
                .size(11);
            let pct_sym = cosmic::iced::widget::text("%")
                .font(cosmic::iced::Font::MONOSPACE)
                .size(8);
            let pct_col = cosmic::iced::widget::column![pct_num, pct_sym]
                .spacing(0)
                .align_x(cosmic::iced::Alignment::Center);
            cosmic::iced::widget::row![cat_image, pct_col]
                .spacing(2)
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
            .button_from_element(content, true);
        // On a horizontal panel the button grows sideways to fit the wide
        // cat (and the label, when shown).
        let btn = if horizontal {
            let content_w = cat_w + if show_pct { 2.0 + PCT_LABEL_WIDTH } else { 0.0 };
            btn.width(Length::Fixed(content_w + 2.0 * pad_major as f32))
        } else {
            btn
        };
        let btn = btn
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    // Popup is open — close it
                    Message::Surface(destroy_popup(id))
                } else {
                    // Popup is closed — create it using the COSMIC popup API.
                    // `app_popup` takes a closure that runs with mutable access
                    // to our applet state (to store the new popup ID).
                    Message::Surface(app_popup::<RunkatApplet>(
                        // live_settings: no live overrides needed
                        |_| Default::default(),
                        move |state: &mut RunkatApplet| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);

                            // Size the popup to fit the rows it will show
                            let core_count = state.cpu_usage.per_core.len().max(
                                state.cpu_frequency.per_core.len().max(
                                    state.cpu_temperature.per_core.len(),
                                ),
                            );
                            let rows = match state.config.animation_source {
                                AnimationSource::Memory => 2,
                                AnimationSource::Gpu => 3 * state.gpu_info.gpus.len().max(1),
                                _ => core_count + 1,
                            };
                            let visible_rows = (rows as u32).min(POPUP_MAX_ROWS);
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
    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

// ---------------------------------------------------------------------------
// Helper methods on RunkatApplet
// ---------------------------------------------------------------------------

impl RunkatApplet {
    /// The selected metric as a 0-100 value for speed, and whether the
    /// runner should be asleep.
    fn current_metric(&self) -> (f32, bool) {
        match self.config.animation_source {
            AnimationSource::CpuUsage => {
                let m = self.smoothed_cpu;
                (m, m < self.config.sleep_threshold_cpu)
            }
            AnimationSource::Frequency => {
                // Speed from % of max frequency, sleep on absolute MHz
                let metric = self.cpu_frequency.average_percentage();
                let avg_mhz = self.cpu_frequency.average_mhz() as f32;
                (metric, avg_mhz < self.config.sleep_threshold_freq)
            }
            AnimationSource::Temperature => {
                // Speed from % of critical temp, sleep on actual degrees
                let actual = self.cpu_temperature.max_temp();
                let metric = self.cpu_temperature.percentage();
                (metric, actual < self.config.sleep_threshold_temp)
            }
            AnimationSource::Memory => {
                let m = self.memory.used_percent();
                (m, m < self.config.sleep_threshold_mem)
            }
            AnimationSource::Gpu => {
                let m = self.smoothed_gpu;
                (m, m < self.config.sleep_threshold_gpu)
            }
        }
    }

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
            AnimationSource::Memory => format!(
                "RAM: {:.0}% ({:.1} / {:.1} GiB)",
                self.memory.used_percent(),
                kib_to_gib(self.memory.used_kb()),
                kib_to_gib(self.memory.total_kb),
            ),
            AnimationSource::Gpu => format!("GPU: {:.0}%", self.smoothed_gpu),
        }
    }

    /// Build the popup content: title, scrollable stats area, status text,
    /// and a "Settings" button.  The stats shown depend on the configured
    /// animation source (CPU usage / frequency / temperature).
    fn popup_content(&self) -> Element<'_, Message> {
        use cosmic::iced::widget::{column, container, row, scrollable, Space};
        use cosmic::iced::{Alignment, Color};

        // Title changes based on what we're monitoring
        let title_text = match self.config.animation_source {
            AnimationSource::CpuUsage => "CPU Usage",
            AnimationSource::Frequency => "CPU Frequency",
            AnimationSource::Temperature => "CPU Temperature",
            AnimationSource::Memory => "Memory Usage",
            AnimationSource::Gpu => "GPU Usage",
        };

        let title_row = row![
            text::body(title_text),
            Space::new().width(Length::Fill), // push title to the left
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

            AnimationSource::Memory => {
                let m = &self.memory;
                let gib = |used: u64, total: u64| {
                    format!("{:.1} / {:.1} GiB", kib_to_gib(used), kib_to_gib(total))
                };
                let mut col = column![row![
                    text::caption("RAM:").width(Length::Fixed(80.0)),
                    self.progress_bar(m.used_percent(), 100.0, false),
                    text::caption(gib(m.used_kb(), m.total_kb)).width(Length::Fixed(110.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center)]
                .spacing(4);
                if m.swap_total_kb > 0 {
                    col = col.push(
                        row![
                            text::caption("Swap:").width(Length::Fixed(80.0)),
                            self.progress_bar(m.swap_percent(), 100.0, false),
                            text::caption(gib(m.swap_used_kb(), m.swap_total_kb))
                                .width(Length::Fixed(110.0)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    );
                }
                col.into()
            }

            AnimationSource::Gpu => {
                let mut col = column![].spacing(4);
                if self.gpu_info.gpus.is_empty() {
                    col = col.push(text::caption("Looking for GPUs..."));
                }
                for gpu in &self.gpu_info.gpus {
                    col = col.push(text::caption(gpu.name.clone()));
                    let load: Element<'_, Message> = match (gpu.asleep, gpu.busy) {
                        (true, _) => text::caption("Powered down (not polled)").into(),
                        (false, Some(busy)) => row![
                            text::caption("Load:").width(Length::Fixed(80.0)),
                            self.progress_bar(busy, 100.0, false),
                            text::caption(format!("{:5.1}%", busy)).width(Length::Fixed(110.0)),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center)
                        .into(),
                        (false, None) => text::caption("Load not available").into(),
                    };
                    col = col.push(load);
                    if let (Some(used), Some(total)) = (gpu.vram_used_mb, gpu.vram_total_mb) {
                        col = col.push(
                            row![
                                text::caption("VRAM:").width(Length::Fixed(80.0)),
                                self.progress_bar(used as f32, total as f32, false),
                                text::caption(format!("{} / {} MB", used, total))
                                    .width(Length::Fixed(110.0)),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                        );
                    }
                }
                col.into()
            }
        };

        // Fun status text describing the runner's current state
        let name = self.config.runner.name();
        let status_text = match self.config.animation_source {
            AnimationSource::Frequency if self.is_sleeping => format!("{name} is idle..."),
            AnimationSource::Frequency => format!("{name} is boosting!"),
            AnimationSource::Temperature if self.is_sleeping => format!("{name} is cool..."),
            AnimationSource::Temperature
                if self.cpu_temperature.max_temp() > TEMP_HOT_THRESHOLD =>
            {
                format!("{name} is HOT!")
            }
            AnimationSource::Temperature => format!("{name} is warm..."),
            _ if self.is_sleeping => format!("{name} is sleeping..."),
            _ => format!("{name} is running!"),
        };

        let bottom_row = row![
            text::caption(status_text),
            Space::new().width(Length::Fill),
            widget::button::standard("Settings...").on_press(Message::OpenSettings),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Make the stats area scrollable for systems with many cores
        let scrollable_stats = scrollable(stats_content)
            .height(Length::Fixed(POPUP_MAX_SCROLL_HEIGHT));

        // Thin horizontal line divider between popup sections
        let divider = || {
            container(Space::new().width(Length::Fill).height(Length::Fixed(1.0))).style(
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
            .into()
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
            container(Space::new().width(Length::Fixed(filled_width)).height(Length::Fixed(BAR_HEIGHT - 2.0)))
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
