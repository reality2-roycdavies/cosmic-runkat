//! COSMIC Panel Applet Module
//!
//! Implements the animated running cat as a native COSMIC panel applet.
//! The cat animation speed varies based on CPU usage, frequency, or temperature.
//! Clicking the applet opens a popup with detailed CPU statistics.

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

/// Application ID (must match desktop entry)
const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat";

/// Messages for the applet
#[derive(Debug, Clone)]
pub enum Message {
    /// Animation frame tick (~33ms)
    AnimationTick,
    /// CPU data update (500ms)
    CpuUpdate,
    /// Config/theme change poll (500ms)
    ConfigCheck,
    /// Popup closed by compositor
    PopupClosed(Id),
    /// Surface action (popup create/destroy)
    Surface(cosmic::surface::Action),
    /// Open the settings window
    OpenSettings,
}

/// Cached sprite resources with theme-aware recoloring
struct SpriteCache {
    /// Original sprites (loaded once from embedded PNGs)
    cat_frames_original: Vec<RgbaImage>,
    cat_sleep_original: RgbaImage,

    /// Recolored sprites (updated on theme change)
    cat_frames_colored: Vec<RgbaImage>,
    cat_sleep_colored: RgbaImage,

    /// Current theme color (for change detection)
    last_theme_color: Option<(u8, u8, u8)>,
}

impl SpriteCache {
    /// Load sprites from embedded resources
    fn load() -> Self {
        let load_img = |data: &[u8]| -> Option<RgbaImage> {
            image::load_from_memory(data).ok().map(|i| i.to_rgba8())
        };

        let cat_sleep = load_img(include_bytes!("../resources/cat-sleep.png"))
            .unwrap_or_else(|| create_fallback_icon(CAT_SIZE));

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
        .map(|opt| opt.unwrap_or_else(|| create_fallback_icon(CAT_SIZE)))
        .collect();

        Self {
            cat_frames_original: cat_frames.clone(),
            cat_sleep_original: cat_sleep.clone(),
            cat_frames_colored: cat_frames,
            cat_sleep_colored: cat_sleep,
            last_theme_color: None,
        }
    }

    /// Recolor sprites if theme color changed
    fn update_colors(&mut self, color: (u8, u8, u8)) {
        if self.last_theme_color == Some(color) {
            return;
        }

        self.cat_frames_colored = self
            .cat_frames_original
            .iter()
            .map(|img| recolor_image(img, color))
            .collect();

        self.cat_sleep_colored = recolor_image(&self.cat_sleep_original, color);
        self.last_theme_color = Some(color);
    }

    /// Get the current frame as an iced image handle
    fn frame_handle(&self, frame: u8, sleeping: bool) -> cosmic::iced::widget::image::Handle {
        let img = if sleeping {
            &self.cat_sleep_colored
        } else {
            &self.cat_frames_colored[frame as usize % self.cat_frames_colored.len()]
        };
        rgba_to_handle(img)
    }
}

/// Recolor all non-transparent pixels to the given color
fn recolor_image(img: &RgbaImage, color: (u8, u8, u8)) -> RgbaImage {
    let (r, g, b) = color;
    let mut result = img.clone();
    for pixel in result.pixels_mut() {
        if pixel[3] > 0 {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
        }
    }
    result
}

/// Convert an RgbaImage to an iced image Handle
fn rgba_to_handle(img: &RgbaImage) -> cosmic::iced::widget::image::Handle {
    cosmic::iced::widget::image::Handle::from_rgba(
        img.width(),
        img.height(),
        img.as_raw().clone(),
    )
}

/// Create a simple fallback circle icon
fn create_fallback_icon(size: u32) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);
    let center = size as f32 / 2.0;
    let radius = size as f32 / 2.5;

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 - center;
        let dy = y as f32 - center;
        if (dx * dx + dy * dy).sqrt() <= radius {
            *pixel = image::Rgba([200, 200, 200, 255]);
        }
    }
    img
}

/// The COSMIC panel applet
pub struct RunkatApplet {
    core: Core,

    // Sprite resources
    sprites: SpriteCache,

    // Animation state
    current_frame: u8,
    is_sleeping: bool,
    last_frame_time: std::time::Instant,

    // CPU monitoring
    cpu_monitor: CpuMonitor,
    cpu_usage: CpuUsage,
    cpu_frequency: CpuFrequency,
    cpu_temperature: CpuTemperature,
    animation_metric: f32,

    // CPU smoothing
    cpu_samples: VecDeque<f32>,
    smoothed_cpu: f32,

    // Config
    config: Config,

    // Popup state
    popup: Option<Id>,

    // Theme
    accent_color: (u8, u8, u8),
    theme_is_dark: bool,

    // Cached tooltip text
    tooltip: String,
}

impl cosmic::Application for RunkatApplet {
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

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let config = Config::load();
        let mut sprites = SpriteCache::load();
        let theme_colors = theme::get_cosmic_theme_colors();
        sprites.update_colors(theme_colors.foreground);

        let cpu_monitor = CpuMonitor::new();
        cpu_monitor.start(CPU_SAMPLE_INTERVAL);

        let applet = Self {
            core,
            sprites,
            current_frame: 0,
            is_sleeping: true,
            last_frame_time: std::time::Instant::now(),
            cpu_monitor,
            cpu_usage: CpuUsage::default(),
            cpu_frequency: CpuFrequency::read(),
            cpu_temperature: CpuTemperature::read(),
            animation_metric: 0.0,
            cpu_samples: VecDeque::with_capacity(CPU_SAMPLE_COUNT),
            smoothed_cpu: 0.0,
            config,
            popup: None,
            accent_color: theme_colors.foreground,
            theme_is_dark: theme_colors.is_dark,
            tooltip: String::from("RunKat"),
        };

        (applet, Task::none())
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::AnimationTick => {
                // Get animation metric based on configured source
                let (metric, sleeping) = match self.config.animation_source {
                    AnimationSource::CpuUsage => {
                        let m = self.smoothed_cpu;
                        (m, m < self.config.sleep_threshold_cpu)
                    }
                    AnimationSource::Frequency => {
                        let freq = &self.cpu_frequency;
                        let metric = freq.average_percentage();
                        let avg_mhz = if freq.per_core.is_empty() {
                            0.0
                        } else {
                            freq.per_core.iter().sum::<u32>() as f32
                                / freq.per_core.len() as f32
                        };
                        (metric, avg_mhz < self.config.sleep_threshold_freq)
                    }
                    AnimationSource::Temperature => {
                        let temp = &self.cpu_temperature;
                        let actual = temp.max_temp();
                        let metric = temp.percentage();
                        (metric, actual < self.config.sleep_threshold_temp)
                    }
                };

                self.animation_metric = metric;
                self.is_sleeping = sleeping;

                // Advance animation frame if running
                if !sleeping {
                    let fps = self.config.calculate_fps(metric);
                    if fps > 0.0 {
                        let frame_duration = Duration::from_secs_f32(1.0 / fps);
                        if self.last_frame_time.elapsed() >= frame_duration {
                            self.current_frame = (self.current_frame + 1) % RUN_FRAMES;
                            self.last_frame_time = std::time::Instant::now();
                        }
                    }
                }
            }

            Message::CpuUpdate => {
                // Read full CPU usage
                self.cpu_usage = self.cpu_monitor.current_full();

                // Smooth aggregate CPU
                let raw = self.cpu_usage.aggregate;
                self.cpu_samples.push_back(raw);
                if self.cpu_samples.len() > CPU_SAMPLE_COUNT {
                    self.cpu_samples.pop_front();
                }
                self.smoothed_cpu = if self.cpu_samples.is_empty() {
                    raw
                } else {
                    self.cpu_samples.iter().sum::<f32>() / self.cpu_samples.len() as f32
                };

                // Update freq and temp
                self.cpu_frequency = CpuFrequency::read();
                self.cpu_temperature = CpuTemperature::read();

                // Update tooltip
                self.tooltip = self.make_tooltip();
            }

            Message::ConfigCheck => {
                // Reload config
                let new_config = Config::load();
                self.config = new_config;

                // Check theme
                let theme_colors = theme::get_cosmic_theme_colors();
                if theme_colors.foreground != self.accent_color
                    || theme_colors.is_dark != self.theme_is_dark
                {
                    self.accent_color = theme_colors.foreground;
                    self.theme_is_dark = theme_colors.is_dark;
                    self.sprites.update_colors(theme_colors.foreground);
                }
            }

            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }

            Message::Surface(action) => {
                return cosmic::task::message(cosmic::Action::Cosmic(
                    cosmic::app::Action::Surface(action),
                ));
            }

            Message::OpenSettings => {
                std::thread::spawn(|| {
                    let exe = std::env::current_exe().unwrap_or_default();
                    let _ = std::process::Command::new(exe).arg("--settings").spawn();
                });
            }
        }

        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        cosmic::iced::Subscription::batch([
            // Animation tick ~30fps
            cosmic::iced::time::every(Duration::from_millis(33))
                .map(|_| Message::AnimationTick),
            // CPU data update
            cosmic::iced::time::every(Duration::from_millis(500))
                .map(|_| Message::CpuUpdate),
            // Config/theme poll
            cosmic::iced::time::every(Duration::from_millis(500))
                .map(|_| Message::ConfigCheck),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        // Render the current cat animation frame as an iced image
        let handle = self.sprites.frame_handle(self.current_frame, self.is_sleeping);
        let suggested = self.core.applet.suggested_size(true);
        let img_size = suggested.0 as f32;

        let cat_image = cosmic::iced::widget::image(handle)
            .width(Length::Fixed(img_size))
            .height(Length::Fixed(img_size));

        // Optionally show CPU% text next to the cat
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

        // Create applet button with click-to-toggle-popup
        let have_popup = self.popup;
        let btn = self
            .core
            .applet
            .button_from_element(content, true)
            .on_press_with_rectangle(move |offset, bounds| {
                if let Some(id) = have_popup {
                    Message::Surface(destroy_popup(id))
                } else {
                    Message::Surface(app_popup::<RunkatApplet>(
                        move |state: &mut RunkatApplet| {
                            let new_id = Id::unique();
                            state.popup = Some(new_id);

                            // Calculate popup size based on content
                            // Fit up to 25 rows (24 cores + 1 summary)
                            let core_count = state.cpu_usage.per_core.len().max(
                                state.cpu_frequency.per_core.len().max(
                                    state.cpu_temperature.per_core.len(),
                                ),
                            );
                            let visible_rows = (core_count + 1).min(25) as u32;
                            let per_row_height = 20u32;
                            let base_height = 100u32; // title + dividers + status + padding
                            let popup_height = base_height + visible_rows * per_row_height;
                            let popup_width = 340u32;

                            let mut popup_settings = state.core.applet.get_popup_settings(
                                state.core.main_window_id().unwrap(),
                                new_id,
                                Some((popup_width, popup_height)),
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
                        Some(Box::new(|state: &RunkatApplet| {
                            Element::from(state.core.applet.popup_container(
                                state.popup_content(),
                            ))
                            .map(cosmic::Action::App)
                        })),
                    ))
                }
            });

        Element::from(self.core.applet.applet_tooltip::<Message>(
            btn,
            self.tooltip.clone(),
            self.popup.is_some(),
            |a| Message::Surface(a),
            None,
        ))
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        // Popup content is rendered via the closure in app_popup, not here
        "".into()
    }

    fn style(&self) -> Option<cosmic::iced_runtime::Appearance> {
        Some(cosmic::applet::style())
    }
}

impl RunkatApplet {
    /// Generate tooltip text
    fn make_tooltip(&self) -> String {
        match self.config.animation_source {
            AnimationSource::CpuUsage => format!("CPU: {:.0}%", self.smoothed_cpu),
            AnimationSource::Frequency => {
                let avg = if self.cpu_frequency.per_core.is_empty() {
                    0
                } else {
                    self.cpu_frequency.per_core.iter().sum::<u32>()
                        / self.cpu_frequency.per_core.len() as u32
                };
                format!("CPU: {} MHz", avg)
            }
            AnimationSource::Temperature => {
                format!("CPU: {:.1}°C", self.cpu_temperature.max_temp())
            }
        }
    }

    /// Build the popup content widget
    fn popup_content(&self) -> widget::Column<'_, Message> {
        use cosmic::iced::widget::{column, container, horizontal_space, row, scrollable, Space};
        use cosmic::iced::{Alignment, Color};

        let title_text = match self.config.animation_source {
            AnimationSource::CpuUsage => "CPU Usage",
            AnimationSource::Frequency => "CPU Frequency",
            AnimationSource::Temperature => "CPU Temperature",
        };

        // Title row
        let title_row = row![
            text::body(title_text),
            horizontal_space(),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Stats content based on animation source
        let stats_content: Element<'_, Message> = match self.config.animation_source {
            AnimationSource::CpuUsage => {
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
                let avg_mhz: u32 = if self.cpu_frequency.per_core.is_empty() {
                    0
                } else {
                    self.cpu_frequency.per_core.iter().sum::<u32>()
                        / self.cpu_frequency.per_core.len() as u32
                };
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

        // Status text
        let status_text = match self.config.animation_source {
            AnimationSource::CpuUsage => {
                if self.cpu_usage.aggregate < self.config.sleep_threshold_cpu {
                    "Cat is sleeping..."
                } else {
                    "Cat is running!"
                }
            }
            AnimationSource::Frequency => {
                let avg_mhz = if self.cpu_frequency.per_core.is_empty() {
                    0.0
                } else {
                    self.cpu_frequency.per_core.iter().sum::<u32>() as f32
                        / self.cpu_frequency.per_core.len() as f32
                };
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
                } else if max_temp > 80.0 {
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

        // Size to fit up to 25 rows (24 cores + 1 total/avg) without scrolling
        let max_scroll_height = 500.0;
        let scrollable_stats = scrollable(stats_content)
            .height(Length::Fixed(max_scroll_height));

        // Divider helper
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

        column![title_row, divider(), scrollable_stats, divider(), bottom_row,]
            .spacing(8)
            .padding(12)
    }

    /// Create a colored progress bar widget
    fn progress_bar(&self, value: f32, max: f32, is_freq: bool) -> Element<'_, Message> {
        use cosmic::iced::widget::{container, Space};
        use cosmic::iced::Color;

        let bar_width = 140.0f32;
        let bar_height = 12.0f32;
        let pct = if max > 0.0 { (value / max).clamp(0.0, 1.0) } else { 0.0 };
        let filled_width = (pct * bar_width).max(0.0).min(bar_width);

        let bar_color = if is_freq {
            Color::from_rgb8(
                (50.0 + pct * 150.0) as u8,
                (100.0 + pct * 100.0) as u8,
                220,
            )
        } else if pct > 0.9 {
            Color::from_rgb8(220, 50, 50)
        } else if pct > 0.7 {
            Color::from_rgb8(220, 150, 50)
        } else if pct > 0.5 {
            Color::from_rgb8(200, 200, 50)
        } else {
            let (r, g, b) = self.accent_color;
            Color::from_rgb8(r, g, b)
        };

        let inner =
            container(Space::new(Length::Fixed(filled_width), Length::Fixed(bar_height - 2.0)))
                .style(move |_: &cosmic::Theme| container::Style {
                    background: Some(cosmic::iced::Background::Color(bar_color)),
                    border: cosmic::iced::Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                });

        container(inner)
            .width(Length::Fixed(bar_width))
            .height(Length::Fixed(bar_height))
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

/// Run the applet
pub fn run_applet() -> cosmic::iced::Result {
    cosmic::applet::run::<RunkatApplet>(())
}
