//! Settings application for cosmic-runkat
//!
//! A libcosmic-based settings window for configuring the running cat indicator.

use cosmic::app::Core;
use cosmic::iced::Length;
use cosmic::widget::{self, settings, text, toggler};
use cosmic::{Action, Application, Element, Task};

use crate::config::{AnimationSource, Config, PopupPosition};

/// Application ID
pub const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat";

/// Messages for the settings application
#[derive(Debug, Clone)]
pub enum Message {
    /// Sleep threshold changed
    SleepThresholdChanged(f32),
    /// Show percentage toggled
    ShowPercentageToggled(bool),
    /// Popup position changed
    PopupPositionChanged(PopupPosition),
    /// Animation source changed
    AnimationSourceChanged(AnimationSource),
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
                self.config.set_current_threshold(value);
                let _ = self.config.save();
            }
            Message::ShowPercentageToggled(value) => {
                self.config.show_percentage = value;
                // Save immediately so tray updates
                let _ = self.config.save();
            }
            Message::PopupPositionChanged(position) => {
                self.config.popup_position = position;
                let _ = self.config.save();
            }
            Message::AnimationSourceChanged(source) => {
                // Each source has its own threshold that persists
                self.config.animation_source = source;
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

        // Build popup position dropdown
        let selected_position_index = PopupPosition::ALL
            .iter()
            .position(|&p| p == self.config.popup_position);
        let position_dropdown = widget::dropdown(
            PopupPosition::NAMES,
            selected_position_index,
            |idx| Message::PopupPositionChanged(PopupPosition::ALL[idx]),
        )
        .width(Length::Fixed(150.0));

        // Build animation source dropdown
        let selected_source_index = AnimationSource::ALL
            .iter()
            .position(|&s| s == self.config.animation_source);
        let source_dropdown = widget::dropdown(
            AnimationSource::NAMES,
            selected_source_index,
            |idx| Message::AnimationSourceChanged(AnimationSource::ALL[idx]),
        )
        .width(Length::Fixed(150.0));

        // Sleep threshold label and range based on animation source
        // For frequency, get max MHz to display threshold in MHz
        let freq_info = crate::sysinfo::CpuFrequency::read();
        let max_freq_mhz = freq_info.max_per_core.first().copied().unwrap_or(5000) as f32;

        let (threshold_label, threshold_range, threshold_unit) = match self.config.animation_source {
            AnimationSource::CpuUsage => ("Sleep Below", 0.0..=30.0, "%"),
            AnimationSource::Frequency => ("Sleep Below", 0.0..=max_freq_mhz, " MHz"),
            AnimationSource::Temperature => ("Sleep Below", 20.0..=100.0, "°C"),
        };

        // Get current threshold for the active mode
        let display_threshold = self.config.current_threshold().clamp(
            *threshold_range.start(),
            *threshold_range.end(),
        );

        // Build main section
        let mut behavior_section = settings::section()
            .title("Behavior")
            .add(settings::item("Monitor", source_dropdown))
            .add(settings::flex_item(
                threshold_label,
                widget::row()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(text::body(format!("{:.0}{}", display_threshold, threshold_unit)))
                    .push(
                        widget::slider(
                            threshold_range,
                            display_threshold,
                            Message::SleepThresholdChanged,
                        )
                        .step(1.0)
                        .width(Length::Fill),
                    ),
            ));

        // Popup position only works in native mode (layer-shell);
        // Flatpak uses a regular window where the compositor controls placement.
        if !crate::paths::is_flatpak() {
            behavior_section = behavior_section
                .add(settings::item("Popup Position", position_dropdown));
        }

        // Only show CPU percentage toggle when monitoring CPU usage
        if self.config.animation_source == AnimationSource::CpuUsage {
            behavior_section = behavior_section.add(settings::item(
                "Show % on Icon",
                toggler(self.config.show_percentage).on_toggle(Message::ShowPercentageToggled),
            ));
        }

        // Use settings::view_column for proper COSMIC styling
        let content = settings::view_column(vec![
            page_title.into(),
            text::caption("The cat runs faster based on the selected metric. Click the tray icon to see details.").into(),
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
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(850.0, 420.0));

    cosmic::app::run::<SettingsApp>(settings, ())
}
