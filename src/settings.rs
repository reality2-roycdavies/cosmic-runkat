//! Settings window for cosmic-runkat
//!
//! A standalone libcosmic application window for configuring the running cat
//! applet.  Launched as a separate process via `cosmic-runkat --settings`
//! so it doesn't block the panel applet's event loop.
//!
//! Changes are saved to disk immediately and the applet picks them up on
//! its next config poll cycle (~500ms).

use cosmic::app::Core;
use cosmic::iced::Length;
use cosmic::widget::{self, settings, text, toggler};
use cosmic::{Action, Application, Element, Task};

use crate::config::{AnimationSource, Config};

/// Application ID for the settings window (separate from the applet ID so
/// the COSMIC compositor treats them as different windows).
const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat.settings";

/// Messages that the settings window can handle.
///
/// Each variant corresponds to a user interaction in the settings UI.
#[derive(Debug, Clone)]
pub enum Message {
    /// The user moved the "sleep below" slider to a new value
    SleepThresholdChanged(f32),
    /// The user toggled the "show percentage" switch
    ShowPercentageToggled(bool),
    /// The user selected a different animation source from the dropdown
    AnimationSourceChanged(AnimationSource),
}

/// State for the settings window.
pub struct SettingsApp {
    /// COSMIC framework core (provides window management, theming, etc.)
    core: Core,
    /// Current configuration — modified in-place and saved on every change
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

    // Empty header sections — the settings window doesn't need header buttons
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        vec![]
    }

    /// Load the current config from disk when the settings window opens.
    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Action<Self::Message>>) {
        let config = Config::load();
        (Self { core, config }, Task::none())
    }

    /// Handle user interactions — each change is saved to disk immediately.
    fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
        match message {
            Message::SleepThresholdChanged(value) => {
                // Update the threshold for whichever animation source is active
                self.config.set_current_threshold(value);
                let _ = self.config.save();
            }
            Message::ShowPercentageToggled(value) => {
                self.config.show_percentage = value;
                let _ = self.config.save();
            }
            Message::AnimationSourceChanged(source) => {
                self.config.animation_source = source;
                let _ = self.config.save();
            }
        }
        Task::none()
    }

    /// Build the settings UI.
    fn view(&self) -> Element<'_, Self::Message> {
        let page_title = text::title1("RunKat Settings");

        // Dropdown to select which metric drives the cat animation
        let selected_source_index = AnimationSource::ALL
            .iter()
            .position(|&s| s == self.config.animation_source);
        let source_dropdown = widget::dropdown(
            AnimationSource::NAMES,
            selected_source_index,
            |idx| Message::AnimationSourceChanged(AnimationSource::ALL[idx]),
        )
        .width(Length::Fixed(150.0));

        // The slider range and units change depending on which source is selected
        let freq_info = crate::sysinfo::CpuFrequency::read();
        let max_freq_mhz = freq_info.max_per_core.first().copied().unwrap_or(5000) as f32;

        let (threshold_label, threshold_range, threshold_unit) =
            match self.config.animation_source {
                AnimationSource::CpuUsage => ("Sleep Below", 0.0..=30.0, "%"),
                AnimationSource::Frequency => ("Sleep Below", 0.0..=max_freq_mhz, " MHz"),
                AnimationSource::Temperature => ("Sleep Below", 20.0..=100.0, "\u{00b0}C"),
            };

        // Clamp the displayed value to the slider range (prevents the slider
        // knob from going off-screen if the config has an out-of-range value)
        let display_threshold = self
            .config
            .current_threshold()
            .clamp(*threshold_range.start(), *threshold_range.end());

        // Build the "Behavior" settings section
        let mut behavior_section = settings::section()
            .title("Behavior")
            .add(settings::item("Monitor", source_dropdown))
            .add(settings::flex_item(
                threshold_label,
                widget::row()
                    .spacing(8)
                    .align_y(cosmic::iced::Alignment::Center)
                    .push(text::body(format!(
                        "{:.0}{}",
                        display_threshold, threshold_unit
                    )))
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

        // The "show percentage on icon" toggle only makes sense in CPU mode
        if self.config.animation_source == AnimationSource::CpuUsage {
            behavior_section = behavior_section.add(settings::item(
                "Show % on Icon",
                toggler(self.config.show_percentage).on_toggle(Message::ShowPercentageToggled),
            ));
        }

        // Layout: centered column with max width for readability
        let content = settings::view_column(vec![
            page_title.into(),
            text::caption(
                "The cat runs faster based on the selected metric. Click the panel applet to see details.",
            )
            .into(),
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

/// Launch the settings window as a standalone COSMIC application.
pub fn run_settings() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(850.0, 380.0));
    cosmic::app::run::<SettingsApp>(settings, ())
}
