//! Settings application for cosmic-runkat
//!
//! A libcosmic-based settings window for configuring the running cat indicator.

use cosmic::app::Core;
use cosmic::iced::Length;
use cosmic::widget::{self, button, column, container, row, text, toggler};
use cosmic::{Action, Application, Element, Task};

use crate::config::Config;

/// Application ID
pub const APP_ID: &str = "io.github.cosmic-runkat.settings";

/// Messages for the settings application
#[derive(Debug, Clone)]
pub enum Message {
    /// Sleep threshold changed
    SleepThresholdChanged(f32),
    /// Show percentage toggled
    ShowPercentageToggled(bool),
    /// Close window
    Close,
    /// Periodic tick for lockfile refresh
    Tick,
}

/// Settings application state
pub struct SettingsApp {
    core: Core,
    config: Config,
}

impl Application for SettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn header_start(&self) -> Vec<Element<Self::Message>> {
        vec![]
    }

    fn header_center(&self) -> Vec<Element<Self::Message>> {
        vec![text::heading("RunKat Settings").into()]
    }

    fn header_end(&self) -> Vec<Element<Self::Message>> {
        vec![]
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Action<Self::Message>>) {
        let config = Config::load();

        (
            Self {
                core,
                config,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
        match message {
            Message::SleepThresholdChanged(value) => {
                self.config.sleep_threshold = value;
                // Save immediately so tray updates
                let _ = self.config.save();
            }
            Message::ShowPercentageToggled(value) => {
                self.config.show_percentage = value;
                // Save immediately so tray updates
                let _ = self.config.save();
            }
            Message::Close => {
                std::process::exit(0);
            }
            Message::Tick => {
                // Refresh the GUI lockfile to indicate we're still running
                crate::create_gui_lockfile();
            }
        }
        Task::none()
    }

    fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        // Refresh lockfile every 30 seconds
        cosmic::iced::time::every(std::time::Duration::from_secs(30))
            .map(|_| Message::Tick)
    }

    fn view(&self) -> Element<Self::Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        // Sleep threshold slider
        let sleep_section = column()
            .spacing(spacing.space_xxs)
            .push(text::body("Sleep Threshold"))
            .push(text::caption(format!(
                "Cat sleeps when CPU is below {:.0}%",
                self.config.sleep_threshold
            )))
            .push(
                widget::slider(0.0..=20.0, self.config.sleep_threshold, Message::SleepThresholdChanged)
                    .step(1.0)
            );

        // Show percentage toggle
        let percentage_section = row()
            .spacing(spacing.space_s)
            .align_y(cosmic::iced::Alignment::Center)
            .push(
                column()
                    .width(Length::Fill)
                    .push(text::body("Show CPU Percentage"))
                    .push(text::caption("Display percentage next to the cat (medium+ panels only)"))
            )
            .push(
                toggler(self.config.show_percentage)
                    .on_toggle(Message::ShowPercentageToggled)
            );

        // Close button
        let buttons = row()
            .spacing(spacing.space_s)
            .push(widget::horizontal_space())
            .push(button::suggested("Close").on_press(Message::Close));

        // Main content
        let content = column()
            .spacing(spacing.space_m)
            .padding(spacing.space_m)
            .push(sleep_section)
            .push(percentage_section)
            .push(widget::vertical_space())
            .push(buttons);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Run the settings application
pub fn run_settings() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default()
        .size(cosmic::iced::Size::new(450.0, 400.0));

    cosmic::app::run::<SettingsApp>(settings, ())
}
