//! Popup window module for displaying CPU statistics
//!
//! Uses Wayland layer-shell protocol via iced's SCTK integration
//! to create a proper dropdown-like popup that appears near the tray icon.

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
    /// Layer surface created
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
    /// Layer surface ID
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
}

impl Default for PopupApp {
    fn default() -> Self {
        let cpu_monitor = CpuMonitor::new();
        cpu_monitor.start(Duration::from_millis(500));

        let theme_colors = theme::get_cosmic_theme_colors();

        Self {
            surface_id: None,
            cpu_monitor,
            cpu_usage: CpuUsage::default(),
            cpu_frequency: CpuFrequency::read(),
            cpu_temperature: CpuTemperature::read(),
            accent_color: theme_colors.foreground,
            should_exit: false,
        }
    }
}

impl PopupApp {
    fn new() -> (Self, Task<Message>) {
        let config = Config::load();

        // Calculate size based on CPU count
        let cpu_count = num_cpus::get();
        let base_height = 180u32;
        let per_core_height = 22u32;
        let height = (base_height + (cpu_count as u32 * per_core_height)).min(700);
        let width = 380u32;

        // Margin from edge (near the tray area)
        let edge_margin = 8;
        // Panel height (typical COSMIC panel)
        let panel_margin = 40;

        // Configure layer surface
        let mut settings = SctkLayerSurfaceSettings::default();
        settings.keyboard_interactivity = KeyboardInteractivity::OnDemand;
        settings.layer = Layer::Overlay; // Above normal windows
        settings.size = Some((Some(width), Some(height)));
        settings.size_limits = Limits::NONE
            .min_width(width as f32)
            .min_height(height as f32)
            .max_width(width as f32)
            .max_height(height as f32);

        // Set anchor and margins based on configured popup position
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

        // Don't reserve exclusive space
        settings.exclusive_zone = -1;

        (Self::default(), get_layer_surface(settings).map(Message::LayerSurfaceCreated))
    }

    fn title(&self, _id: Id) -> String {
        String::from("RunKat CPU Monitor")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LayerSurfaceCreated(id) => {
                self.surface_id = Some(id);
            }
            Message::Close => {
                self.should_exit = true;
                if let Some(id) = self.surface_id {
                    return destroy_layer_surface(id);
                }
            }
            Message::OpenSettings => {
                std::thread::spawn(|| {
                    let exe = std::env::current_exe().unwrap_or_default();
                    let _ = Command::new(exe).arg("--settings").spawn();
                });
                self.should_exit = true;
                if let Some(id) = self.surface_id {
                    return destroy_layer_surface(id);
                }
            }
            Message::Tick => {
                self.cpu_usage = self.cpu_monitor.current_full();
                self.cpu_frequency = CpuFrequency::read();
                self.cpu_temperature = CpuTemperature::read();
                let theme_colors = theme::get_cosmic_theme_colors();
                self.accent_color = theme_colors.foreground;
            }
            Message::Event(event) => {
                // Close on focus lost or Escape key
                match event {
                    Event::Window(window::Event::Unfocused) => {
                        self.should_exit = true;
                        if let Some(id) = self.surface_id {
                            return destroy_layer_surface(id);
                        }
                    }
                    Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                        ..
                    }) => {
                        self.should_exit = true;
                        if let Some(id) = self.surface_id {
                            return destroy_layer_surface(id);
                        }
                    }
                    _ => {}
                }
            }
        }

        if self.should_exit && self.surface_id.is_none() {
            std::process::exit(0);
        }

        Task::none()
    }

    fn view(&self, _id: Id) -> Element<'_, Message> {
        let config = Config::load();

        // Title based on animation source
        let title_text = match config.animation_source {
            AnimationSource::CpuUsage => "CPU Usage",
            AnimationSource::Frequency => "CPU Frequency",
            AnimationSource::Temperature => "CPU Temperature",
        };

        // Title row with close button
        let title_row = row![
            text(title_text).size(16),
            horizontal_space(),
            button(text("×").size(18))
                .on_press(Message::Close)
                .padding([2, 8])
                .style(button::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Build content based on animation source
        let stats_content: Element<'_, Message> = match config.animation_source {
            AnimationSource::CpuUsage => {
                // Overall CPU bar
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

                // Per-core CPU bars
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
                // Average frequency
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

                // Per-core frequencies
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

                // Max temperature
                let max_row = row![
                    text("Max:").size(14).width(Length::Fixed(80.0)),
                    self.progress_bar(max_temp, critical, false),
                    text(format!("{:.1}°C", max_temp))
                        .size(14)
                        .width(Length::Fixed(55.0)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                let mut temp_column = column![max_row].spacing(2);

                // Package temperature
                if let Some(pkg_temp) = self.cpu_temperature.package {
                    let pkg_row = row![
                        text("Package:").size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(pkg_temp, critical, false),
                        text(format!("{:.1}°C", pkg_temp))
                            .size(11)
                            .width(Length::Fixed(55.0)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    temp_column = temp_column.push(pkg_row);
                }

                // Per-core temps
                for (i, &temp) in self.cpu_temperature.per_core.iter().enumerate() {
                    let label = format!("Core {}:", i);
                    let core_row = row![
                        text(label).size(11).width(Length::Fixed(80.0)),
                        self.progress_bar(temp, critical, false),
                        text(format!("{:.1}°C", temp))
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

        // Status text (using per-source thresholds)
        let status_text = match config.animation_source {
            AnimationSource::CpuUsage => {
                if self.cpu_usage.aggregate < config.sleep_threshold_cpu {
                    "Cat is sleeping..."
                } else {
                    "Cat is running!"
                }
            }
            AnimationSource::Frequency => {
                // Compare average MHz against threshold in MHz
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

        // Bottom row with status and settings button
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

        // Divider helper
        let divider = || {
            container(Space::new(Length::Fill, Length::Fixed(1.0)))
                .style(|_: &Theme| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgba(0.5, 0.5, 0.5, 0.3))),
                    ..Default::default()
                })
        };

        // Scrollable stats area
        let scrollable_stats = scrollable(stats_content)
            .height(Length::Fill);

        // Main content
        let content = column![
            title_row,
            divider(),
            scrollable_stats,
            divider(),
            bottom_row,
        ]
        .spacing(8)
        .padding(12);

        // Wrap in styled container
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

    /// Create a progress bar
    fn progress_bar(&self, value: f32, max: f32, is_freq: bool) -> Element<'_, Message> {
        let bar_width = 140.0f32;
        let bar_height = 12.0f32;
        let pct = if max > 0.0 { value / max } else { 0.0 };
        let pct = pct.clamp(0.0, 1.0);
        let filled_width = (pct * bar_width).max(0.0).min(bar_width);

        // Color based on value
        let bar_color = if is_freq {
            // Blue for frequency
            Color::from_rgb8(
                (50.0 + pct * 150.0) as u8,
                (100.0 + pct * 100.0) as u8,
                220,
            )
        } else if pct > 0.9 {
            Color::from_rgb8(220, 50, 50) // Red
        } else if pct > 0.7 {
            Color::from_rgb8(220, 150, 50) // Orange
        } else if pct > 0.5 {
            Color::from_rgb8(200, 200, 50) // Yellow
        } else {
            let (r, g, b) = self.accent_color;
            Color::from_rgb8(r, g, b)
        };

        // Inner filled bar
        let inner = container(Space::new(Length::Fixed(filled_width), Length::Fixed(bar_height - 2.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(iced::Background::Color(bar_color)),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

        // Outer background bar
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
    // Check /proc for processes with --popup argument
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
        // Skip our own process
        if pid == current_pid {
            continue;
        }
        // Check cmdline for --popup
        let cmdline_path = format!("/proc/{}/cmdline", pid);
        if let Ok(cmdline) = std::fs::read_to_string(&cmdline_path) {
            if cmdline.contains("cosmic-runkat") && cmdline.contains("--popup") {
                tracing::debug!("Found existing popup process: PID {}", pid);
                return true;
            }
        }
    }
    false
}

/// Run the popup using layer-shell
pub fn run_popup() -> iced::Result {
    // Check if popup is already running
    if is_popup_running() {
        eprintln!("Popup is already running");
        return Ok(());
    }

    iced::daemon(PopupApp::title, PopupApp::update, PopupApp::view)
        .subscription(PopupApp::subscription)
        .run_with(PopupApp::new)
}
