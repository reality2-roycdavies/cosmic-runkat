//! Popup window module for displaying CPU statistics
//!
//! Uses Wayland layer-shell protocol via iced's SCTK integration
//! to create a proper dropdown-like popup that appears near the tray icon.
//! Falls back to a regular window in Flatpak where layer-shell is unavailable.

use crate::config::{AnimationSource, Config, PopupPosition};
use crate::cpu::{CpuMonitor, CpuUsage};
use crate::sysinfo::{CpuFrequency, CpuTemperature};
use crate::theme;

use cosmic::iced;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity, Layer,
};

use cosmic::iced_core::layout::Limits;
use cosmic::iced::event::{self, Event};
use cosmic::iced::widget::{button, column, container, horizontal_space, row, scrollable, text, Space};
use cosmic::iced::window::{self, Id};
use cosmic::iced::{Alignment, Color, Element, Length, Subscription, Task, Theme};

use std::process::Command;
use std::time::Duration;

/// Messages for the popup application
#[derive(Debug, Clone)]
pub enum Message {
    /// Layer surface created (layer-shell mode only)
    LayerSurfaceCreated(Id),
    /// Close the popup
    Close,
    /// Open settings
    OpenSettings,
    /// Tick for CPU updates
    Tick,
    /// Window event (for focus tracking)
    Event(Event),
}

/// Popup application state
struct PopupApp {
    /// Whether running in windowed mode (Flatpak fallback)
    windowed: bool,
    /// Layer surface ID (None in windowed mode)
    surface_id: Option<Id>,
    /// CPU monitor for live updates
    cpu_monitor: CpuMonitor,
    /// Current CPU usage data
    cpu_usage: CpuUsage,
    /// Current CPU frequency data
    cpu_frequency: CpuFrequency,
    /// Current CPU temperature data
    cpu_temperature: CpuTemperature,
    /// Theme accent color
    accent_color: (u8, u8, u8),
    /// Whether we should exit
    should_exit: bool,
    /// Tick count for auto-exit safety (exits if surface never created)
    tick_count: u32,
}

impl Default for PopupApp {
    fn default() -> Self {
        let cpu_monitor = CpuMonitor::new();
        cpu_monitor.start(Duration::from_millis(500));

        let theme_colors = theme::get_cosmic_theme_colors();

        Self {
            windowed: false,
            surface_id: None,
            cpu_monitor,
            cpu_usage: CpuUsage::default(),
            cpu_frequency: CpuFrequency::read(),
            cpu_temperature: CpuTemperature::read(),
            accent_color: theme_colors.foreground,
            should_exit: false,
            tick_count: 0,
        }
    }
}

impl PopupApp {
    /// Initialize for layer-shell mode (native)
    fn new_layer_shell() -> (Self, Task<Message>) {
        let config = Config::load();

        let cpu_count = num_cpus::get();
        let base_height = 180u32;
        let per_core_height = 22u32;
        let height = (base_height + (cpu_count as u32 * per_core_height)).min(700);
        let width = 380u32;

        let edge_margin = 8;
        let panel_margin = 40;

        let mut settings = SctkLayerSurfaceSettings::default();
        settings.keyboard_interactivity = KeyboardInteractivity::OnDemand;
        settings.layer = Layer::Overlay;
        settings.size = Some((Some(width), Some(height)));
        settings.size_limits = Limits::NONE
            .min_width(width as f32)
            .min_height(height as f32)
            .max_width(width as f32)
            .max_height(height as f32);

        let (anchor, margin) = match config.popup_position {
            PopupPosition::TopLeft => (
                Anchor::TOP | Anchor::LEFT,
                cosmic::iced::platform_specific::runtime::wayland::layer_surface::IcedMargin {
                    top: panel_margin,
                    right: 0,
                    bottom: 0,
                    left: edge_margin,
                },
            ),
            PopupPosition::TopRight => (
                Anchor::TOP | Anchor::RIGHT,
                cosmic::iced::platform_specific::runtime::wayland::layer_surface::IcedMargin {
                    top: panel_margin,
                    right: edge_margin,
                    bottom: 0,
                    left: 0,
                },
            ),
            PopupPosition::BottomLeft => (
                Anchor::BOTTOM | Anchor::LEFT,
                cosmic::iced::platform_specific::runtime::wayland::layer_surface::IcedMargin {
                    top: 0,
                    right: 0,
                    bottom: panel_margin,
                    left: edge_margin,
                },
            ),
            PopupPosition::BottomRight => (
                Anchor::BOTTOM | Anchor::RIGHT,
                cosmic::iced::platform_specific::runtime::wayland::layer_surface::IcedMargin {
                    top: 0,
                    right: edge_margin,
                    bottom: panel_margin,
                    left: 0,
                },
            ),
        };

        settings.anchor = anchor;
        settings.margin = margin;
        settings.exclusive_zone = -1;

        (Self::default(), get_layer_surface(settings).map(Message::LayerSurfaceCreated))
    }

    /// Initialize for windowed mode (Flatpak fallback)
    fn new_windowed() -> (Self, Task<Message>) {
        let mut app = Self::default();
        app.windowed = true;
        (app, Task::none())
    }

    /// Shared title
    fn popup_title(&self) -> String {
        String::from("RunKat CPU Monitor")
    }

    // -- Daemon mode (layer-shell) function signatures --

    fn title_daemon(&self, _id: Id) -> String {
        self.popup_title()
    }

    fn view_daemon(&self, _id: Id) -> Element<'_, Message> {
        self.popup_view()
    }

    // -- Application mode (windowed) function signatures --

    fn title_windowed(&self) -> String {
        self.popup_title()
    }

    fn view_windowed(&self) -> Element<'_, Message> {
        self.popup_view()
    }

    // -- Shared logic --

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LayerSurfaceCreated(id) => {
                self.surface_id = Some(id);
            }
            Message::Close => {
                self.should_exit = true;
                if let Some(id) = self.surface_id.take() {
                    return destroy_layer_surface(id);
                }
                std::process::exit(0);
            }
            Message::OpenSettings => {
                std::thread::spawn(|| {
                    let exe = std::env::current_exe().unwrap_or_default();
                    let _ = Command::new(exe).arg("--settings").spawn();
                });
                self.should_exit = true;
                if let Some(id) = self.surface_id.take() {
                    return destroy_layer_surface(id);
                }
                std::process::exit(0);
            }
            Message::Tick => {
                if self.should_exit {
                    std::process::exit(0);
                }

                self.tick_count += 1;
                // Only auto-exit for layer-shell mode where surface might fail to create
                if !self.windowed && self.tick_count > 10 && self.surface_id.is_none() {
                    tracing::warn!("Popup surface never created, exiting");
                    std::process::exit(1);
                }

                self.cpu_usage = self.cpu_monitor.current_full();
                self.cpu_frequency = CpuFrequency::read();
                self.cpu_temperature = CpuTemperature::read();
                let theme_colors = theme::get_cosmic_theme_colors();
                self.accent_color = theme_colors.foreground;
            }
            Message::Event(event) => {
                match event {
                    Event::Window(window::Event::Unfocused) if !self.windowed => {
                        // Only auto-close on unfocus in layer-shell mode (popup overlay)
                        self.should_exit = true;
                        if let Some(id) = self.surface_id.take() {
                            return destroy_layer_surface(id);
                        }
                        std::process::exit(0);
                    }
                    Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                        ..
                    }) => {
                        self.should_exit = true;
                        if let Some(id) = self.surface_id.take() {
                            return destroy_layer_surface(id);
                        }
                        std::process::exit(0);
                    }
                    _ => {}
                }
            }
        }

        Task::none()
    }

    fn popup_view(&self) -> Element<'_, Message> {
        let config = Config::load();

        let title_text = match config.animation_source {
            AnimationSource::CpuUsage => "CPU Usage",
            AnimationSource::Frequency => "CPU Frequency",
            AnimationSource::Temperature => "CPU Temperature",
        };

        let title_row = row![
            text(title_text).size(16),
            horizontal_space(),
            button(text("\u{00d7}").size(18))
                .on_press(Message::Close)
                .padding([2, 8])
                .style(button::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let stats_content: Element<'_, Message> = match config.animation_source {
            AnimationSource::CpuUsage => {
                let overall_pct = self.cpu_usage.aggregate;
                let overall_row = row![
                    text("Total:").size(14).width(Length::Fixed(80.0)),
                    self.progress_bar(overall_pct, 100.0, false),
                    text(format!("{:5.1}%", overall_pct))
                        .size(14)
                        .width(Length::Fixed(55.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut core_column = column![].spacing(2);
                for (i, &pct) in self.cpu_usage.per_core.iter().enumerate() {
                    let label = format!("CPU{}:", i);
                    let core_row = row![
                        text(label).size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(pct, 100.0, false),
                        text(format!("{:5.1}%", pct))
                            .size(11)
                            .width(Length::Fixed(55.0)),
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
                    text("Avg:").size(14).width(Length::Fixed(80.0)),
                    self.progress_bar(avg_mhz as f32, max_mhz as f32, true),
                    text(format!("{} MHz", avg_mhz))
                        .size(14)
                        .width(Length::Fixed(80.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut core_column = column![].spacing(2);
                for (i, &mhz) in self.cpu_frequency.per_core.iter().enumerate() {
                    let max = self.cpu_frequency.max_per_core.get(i).copied().unwrap_or(1);
                    let label = format!("CPU{}:", i);
                    let core_row = row![
                        text(label).size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(mhz as f32, max as f32, true),
                        text(format!("{} MHz", mhz))
                            .size(11)
                            .width(Length::Fixed(80.0)),
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
                    text("Max:").size(14).width(Length::Fixed(80.0)),
                    self.progress_bar(max_temp, critical, false),
                    text(format!("{:.1}\u{00b0}C", max_temp))
                        .size(14)
                        .width(Length::Fixed(55.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut temp_column = column![max_row].spacing(2);

                if let Some(pkg_temp) = self.cpu_temperature.package {
                    let pkg_row = row![
                        text("Package:").size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(pkg_temp, critical, false),
                        text(format!("{:.1}\u{00b0}C", pkg_temp))
                            .size(11)
                            .width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    temp_column = temp_column.push(pkg_row);
                }

                for (i, &temp) in self.cpu_temperature.per_core.iter().enumerate() {
                    let label = format!("Core {}:", i);
                    let core_row = row![
                        text(label).size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(temp, critical, false),
                        text(format!("{:.1}\u{00b0}C", temp))
                            .size(11)
                            .width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    temp_column = temp_column.push(core_row);
                }

                temp_column.into()
            }
        };

        let status_text = match config.animation_source {
            AnimationSource::CpuUsage => {
                if self.cpu_usage.aggregate < config.sleep_threshold_cpu {
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
                if avg_mhz < config.sleep_threshold_freq {
                    "Cat is idle..."
                } else {
                    "Cat is boosting!"
                }
            }
            AnimationSource::Temperature => {
                let max_temp = self.cpu_temperature.max_temp();
                if max_temp < config.sleep_threshold_temp {
                    "Cat is cool..."
                } else if max_temp > 80.0 {
                    "Cat is HOT!"
                } else {
                    "Cat is warm..."
                }
            }
        };

        let bottom_row = row![
            text(status_text).size(12),
            horizontal_space(),
            button(text("Settings").size(12))
                .on_press(Message::OpenSettings)
                .padding([4, 12])
                .style(button::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let divider = || {
            container(Space::new(Length::Fill, Length::Fixed(1.0)))
                .style(|_: &Theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.3))),
                    ..Default::default()
                })
        };

        let scrollable_stats = scrollable(stats_content)
            .height(Length::Fill);

        let content = column![
            title_row,
            divider(),
            scrollable_stats,
            divider(),
            bottom_row,
        ]
        .spacing(8)
        .padding(12);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_: &Theme| {
                let bg_color = if theme::get_cosmic_theme_colors().is_dark {
                    Color::from_rgb8(40, 40, 45)
                } else {
                    Color::from_rgb8(250, 250, 252)
                };

                container::Style {
                    background: Some(iced::Background::Color(bg_color)),
                    border: iced::Border {
                        color: Color::from_rgba(0.5, 0.5, 0.5, 0.5),
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 16.0,
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(500)).map(|_| Message::Tick),
            event::listen().map(Message::Event),
        ])
    }

    fn progress_bar(&self, value: f32, max: f32, is_freq: bool) -> Element<'_, Message> {
        let bar_width = 140.0f32;
        let bar_height = 12.0f32;
        let pct = if max > 0.0 { value / max } else { 0.0 };
        let pct = pct.clamp(0.0, 1.0);
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

        let inner = container(Space::new(Length::Fixed(filled_width), Length::Fixed(bar_height - 2.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(bar_color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        container(inner)
            .width(Length::Fixed(bar_width))
            .height(Length::Fixed(bar_height))
            .style(|_: &Theme| container::Style {
                background: Some(iced::Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.2))),
                border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    }
}

/// Check if popup is already running by looking for the process
pub fn is_popup_running() -> bool {
    let current_pid = std::process::id();
    let entries = match std::fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!("Cannot read /proc: {}, assuming popup not running", e);
            return false;
        }
    };

    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == current_pid {
            continue;
        }
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let Ok(cmdline_bytes) = std::fs::read(&cmdline_path) else {
            continue;
        };
        let args: Vec<&[u8]> = cmdline_bytes.split(|&b| b == 0).collect();
        let Some(exe_arg) = args.first() else { continue };
        let exe_str = String::from_utf8_lossy(exe_arg);
        if !exe_str.ends_with("cosmic-runkat") {
            continue;
        }
        let has_popup = args.iter().any(|arg| arg == b"--popup" || arg == b"-p");
        if has_popup {
            tracing::debug!("Found existing popup process: PID {} ({})", pid, exe_str);
            return true;
        }
    }
    false
}

/// Run the popup - uses layer-shell natively, regular window in Flatpak
pub fn run_popup() -> iced::Result {
    if is_popup_running() {
        eprintln!("Popup is already running");
        return Ok(());
    }

    if crate::paths::is_flatpak() {
        tracing::info!("Running popup in windowed mode (Flatpak)");
        run_popup_windowed()
    } else {
        tracing::info!("Running popup in layer-shell mode (native)");
        run_popup_layer_shell()
    }
}

/// Layer-shell popup (native, positioned as overlay near tray)
fn run_popup_layer_shell() -> iced::Result {
    iced::daemon(PopupApp::title_daemon, PopupApp::update, PopupApp::view_daemon)
        .subscription(PopupApp::subscription)
        .run_with(PopupApp::new_layer_shell)
}

/// Regular window popup (Flatpak fallback)
fn run_popup_windowed() -> iced::Result {
    let cpu_count = num_cpus::get();
    let height = (180 + cpu_count as u32 * 22).min(700);

    let size = iced::Size::new(380.0, height as f32);
    let window_settings = iced::window::Settings {
        size,
        min_size: Some(size),
        max_size: Some(size),
        resizable: false,
        decorations: false,
        ..Default::default()
    };

    iced::application(PopupApp::title_windowed, PopupApp::update, PopupApp::view_windowed)
        .subscription(PopupApp::subscription)
        .window(window_settings)
        .run_with(PopupApp::new_windowed)
}
