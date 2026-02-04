//! Popup window module for displaying per-core CPU statistics
//!
//! Uses Wayland layer-shell protocol via iced's SCTK integration
//! to create a proper dropdown-like popup that appears near the tray icon.

use crate::config::{Config, PopupPosition};
use crate::cpu::{CpuMonitor, CpuUsage};
use crate::theme;

use cosmic::iced;
use cosmic::iced::platform_specific::runtime::wayland::layer_surface::SctkLayerSurfaceSettings;
use cosmic::iced::platform_specific::shell::commands::layer_surface::{
    destroy_layer_surface, get_layer_surface, Anchor, KeyboardInteractivity, Layer,
};

use cosmic::iced_core::layout::Limits;
use cosmic::iced::event::{self, Event};
use cosmic::iced::widget::{button, column, container, horizontal_space, row, text, Space};
use cosmic::iced::window::{self, Id};
use cosmic::iced::{Alignment, Color, Element, Length, Subscription, Task, Theme};

use std::fs;
use std::io::Write;
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
        let per_core_height = 20u32;
        let height = (base_height + (cpu_count as u32 * per_core_height)).min(600);
        let width = 320u32;

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
        // Note: ksni on Wayland provides x=0, y=0 (no global coordinates available)
        // so we use a configurable position instead
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
            // Clean up lockfile before exiting
            remove_popup_lockfile();
            std::process::exit(0);
        }

        Task::none()
    }

    fn view(&self, _id: Id) -> Element<'_, Message> {
        let config = Config::load();

        // Title row with close button
        let title_row = row![
            text("RunKat CPU Monitor").size(16),
            horizontal_space(),
            button(text("×").size(18))
                .on_press(Message::Close)
                .padding([2, 8])
                .style(button::secondary),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Overall CPU bar
        let overall_pct = self.cpu_usage.aggregate;
        let overall_row = row![
            text("Total:").size(14).width(Length::Fixed(55.0)),
            self.cpu_bar(overall_pct, true),
            text(format!("{:5.1}%", overall_pct))
                .size(14)
                .width(Length::Fixed(55.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        // Per-core CPU bars
        let mut core_column = column![].spacing(2);
        for (i, &pct) in self.cpu_usage.per_core.iter().enumerate() {
            let label = if self.cpu_usage.per_core.len() > 8 {
                format!("{:2}", i)
            } else {
                format!("CPU{}", i)
            };

            let core_row = row![
                text(label).size(11).width(Length::Fixed(55.0)),
                self.cpu_bar(pct, false),
                text(format!("{:5.1}%", pct))
                    .size(11)
                    .width(Length::Fixed(55.0)),
            ]
            .spacing(8)
            .align_y(Alignment::Center);

            core_column = core_column.push(core_row);
        }

        // Status text
        let status_text = if overall_pct < config.sleep_threshold {
            "Cat is sleeping..."
        } else {
            "Cat is running!"
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

        // Main content
        let content = column![
            title_row,
            divider(),
            overall_row,
            divider(),
            core_column,
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

    /// Create a CPU usage bar
    fn cpu_bar(&self, percent: f32, is_total: bool) -> Element<'_, Message> {
        let bar_width = 140.0f32;
        let bar_height = if is_total { 14.0f32 } else { 10.0f32 };
        let filled_width = (percent / 100.0 * bar_width).max(0.0).min(bar_width);

        // Color based on usage
        let bar_color = if percent > 90.0 {
            Color::from_rgb8(220, 50, 50)
        } else if percent > 70.0 {
            Color::from_rgb8(220, 150, 50)
        } else if percent > 50.0 {
            Color::from_rgb8(200, 200, 50)
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

/// Get popup lockfile path
fn popup_lockfile() -> std::path::PathBuf {
    crate::paths::app_config_dir().join("popup.lock")
}

/// Create popup lockfile
fn create_popup_lockfile() {
    let path = popup_lockfile();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = fs::File::create(&path) {
        let _ = write!(file, "{}", std::process::id());
    }
}

/// Remove popup lockfile
fn remove_popup_lockfile() {
    let _ = fs::remove_file(popup_lockfile());
}

/// Run the popup using layer-shell
pub fn run_popup() -> iced::Result {
    // Create lockfile to prevent multiple instances
    create_popup_lockfile();

    let result = iced::daemon(PopupApp::title, PopupApp::update, PopupApp::view)
        .subscription(PopupApp::subscription)
        .run_with(PopupApp::new);

    // Clean up lockfile on exit
    remove_popup_lockfile();
    result
}
