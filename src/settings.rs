//! Settings application for cosmic-runkat
//!
//! A libcosmic-based settings window for configuring the running cat indicator.

use cosmic::app::Core;
use cosmic::iced::Length;
use cosmic::widget::{self, settings, text, toggler};
use cosmic::{Action, Application, Element, Task};

use crate::config::Config;

/// Application ID
pub const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat";

/// Messages for the settings application
#[derive(Debug, Clone)]
pub enum Message {
    /// Sleep threshold changed
    SleepThresholdChanged(f32),
    /// Show percentage toggled
    ShowPercentageToggled(bool),
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

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Action<Self::Message>>) {
        let config = Config::load();

        (Self { core, config }, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
        match message {
            Message::SleepThresholdChanged(value) => {
                self.config.sleep_threshold = value;
                // Validate and save immediately so tray updates
                if let Err(e) = self.config.validate() {
                    eprintln!("Warning: Invalid config change: {}", e);
                    // Optionally revert or show error to user in UI
                } else {
                    let _ = self.config.save();
                }
            }
            Message::ShowPercentageToggled(value) => {
                self.config.show_percentage = value;
                // Save immediately so tray updates
                let _ = self.config.save();
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
        cosmic::iced::time::every(std::time::Duration::from_secs(30)).map(|_| Message::Tick)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        // Page title (large, like COSMIC Settings)
        let page_title = text::title1("RunKat Settings");

        // Build sections using COSMIC settings widgets
        let behavior_section = settings::section()
            .title("Behavior")
            .add(settings::item(
                "Show CPU Percentage",
                toggler(self.config.show_percentage).on_toggle(Message::ShowPercentageToggled),
            ))
            .add(settings::flex_item(
                "Sleep Threshold",
                widget::row()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(text::body(format!("{:.0}%", self.config.sleep_threshold)))
                    .push(
                        widget::slider(
                            0.0..=20.0,
                            self.config.sleep_threshold,
                            Message::SleepThresholdChanged,
                        )
                        .step(1.0)
                        .width(Length::Fill),
                    ),
            ));

        // Use settings::view_column for proper COSMIC styling
        let content = settings::view_column(vec![
            page_title.into(),
            text::caption("Display percentage next to the cat on medium+ panels. Cat sleeps when CPU is below the threshold.").into(),
            behavior_section.into(),
        ]);

        widget::container(widget::container(content).max_width(800))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .padding(24)
            .into()
    }
}

/// Run the settings application
pub fn run_settings() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(850.0, 400.0));

    cosmic::app::run::<SettingsApp>(settings, ())
}
