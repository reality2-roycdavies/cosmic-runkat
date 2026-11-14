//! Embeddable settings page for cosmic-runkat
//!
//! Provides the settings UI as standalone State/Message/init/update/view
//! functions that can be embedded in cosmic-applet-settings or wrapped
//! in a standalone Application window.

use cosmic::iced::Length;
use cosmic::widget::{self, settings, text, toggler};
use cosmic::Element;

use crate::config::{AnimationSource, Config, Runner};

pub struct State {
    pub config: Config,
    pub max_freq_mhz: f32,
    /// What is in the "Sleep Below" number box.  Kept separately from the
    /// config so half-typed values like "" or "7." can sit in the box.
    pub threshold_text: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    SleepThresholdChanged(f32),
    SleepThresholdTyped(String),
    ShowPercentageToggled(bool),
    AnimationSourceChanged(AnimationSource),
    RunnerChanged(Runner),
}

pub fn init() -> State {
    let config = Config::load();
    let freq_info = crate::sysinfo::CpuFrequency::read();
    let max_freq_mhz = freq_info.max_per_core.first().copied().unwrap_or(5000) as f32;

    let threshold_text = format_threshold(config.current_threshold());
    State { config, max_freq_mhz, threshold_text }
}

/// "10" for whole numbers, "7.5" otherwise
fn format_threshold(value: f32) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

/// Values a typed threshold may take for each source (same limits as
/// `Config::validate`).  Wider than the slider, which covers the useful range.
fn typed_range(source: AnimationSource) -> std::ops::RangeInclusive<f32> {
    match source {
        AnimationSource::Frequency => 0.0..=10000.0,
        AnimationSource::Temperature => 0.0..=150.0,
        AnimationSource::CpuUsage | AnimationSource::Memory | AnimationSource::Gpu => {
            0.0..=100.0
        }
    }
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::SleepThresholdChanged(value) => {
            state.config.set_current_threshold(value);
            state.threshold_text = format_threshold(value);
            let _ = state.config.save();
        }
        Message::SleepThresholdTyped(text) => {
            // Save as soon as the box holds a valid number in range;
            // anything else just stays in the box until it is fixed.
            if let Ok(value) = text.trim().parse::<f32>() {
                if typed_range(state.config.animation_source).contains(&value) {
                    state.config.set_current_threshold(value);
                    let _ = state.config.save();
                }
            }
            state.threshold_text = text;
        }
        Message::ShowPercentageToggled(value) => {
            state.config.show_percentage = value;
            let _ = state.config.save();
        }
        Message::AnimationSourceChanged(source) => {
            state.config.animation_source = source;
            state.threshold_text = format_threshold(state.config.current_threshold());
            let _ = state.config.save();
        }
        Message::RunnerChanged(runner) => {
            state.config.runner = runner;
            let _ = state.config.save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State {
            config: Config::default(),
            max_freq_mhz: 5000.0,
            threshold_text: "5".into(),
        }
    }

    #[test]
    fn typing_a_valid_number_sets_threshold() {
        let mut s = state();
        s.config.animation_source = AnimationSource::CpuUsage;
        // update() saves to disk; only exercise the parsing path here
        let value: f32 = "42.5".parse().unwrap();
        assert!(typed_range(s.config.animation_source).contains(&value));
        s.config.set_current_threshold(value);
        assert_eq!(s.config.sleep_threshold_cpu, 42.5);
    }

    #[test]
    fn out_of_range_and_junk_are_rejected() {
        let r = typed_range(AnimationSource::CpuUsage);
        assert!(!r.contains(&150.0));
        assert!("abc".parse::<f32>().is_err());
        assert!(typed_range(AnimationSource::Frequency).contains(&3200.0));
    }

    #[test]
    fn formats_whole_and_fractional_values() {
        assert_eq!(format_threshold(10.0), "10");
        assert_eq!(format_threshold(7.5), "7.5");
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let page_title = text::title1("RunKat Settings");

    let selected_source_index = AnimationSource::ALL
        .iter()
        .position(|&s| s == state.config.animation_source);
    let source_dropdown = widget::dropdown(
        AnimationSource::NAMES,
        selected_source_index,
        |idx| Message::AnimationSourceChanged(AnimationSource::ALL[idx]),
    )
    .width(Length::Fixed(150.0));

    let (threshold_label, threshold_range, threshold_unit) =
        match state.config.animation_source {
            AnimationSource::CpuUsage => ("Sleep Below", 0.0..=100.0, "%"),
            AnimationSource::Frequency => ("Sleep Below", 0.0..=state.max_freq_mhz, " MHz"),
            AnimationSource::Temperature => ("Sleep Below", 20.0..=100.0, "\u{00b0}C"),
            AnimationSource::Memory => ("Sleep Below", 0.0..=100.0, "%"),
            AnimationSource::Gpu => ("Sleep Below", 0.0..=100.0, "%"),
        };

    let runner_dropdown = widget::dropdown(
        Runner::NAMES,
        Runner::ALL.iter().position(|&r| r == state.config.runner),
        |idx| Message::RunnerChanged(Runner::ALL[idx]),
    )
    .width(Length::Fixed(150.0));

    let display_threshold = state
        .config
        .current_threshold()
        .clamp(*threshold_range.start(), *threshold_range.end());

    let mut behavior_section = settings::section()
        .title("Behavior")
        .add(settings::item("Runner", runner_dropdown))
        .add(settings::item("Monitor", source_dropdown))
        .add(settings::flex_item(
            threshold_label,
            widget::Row::new()
                .spacing(8)
                .align_y(cosmic::iced::Alignment::Center)
                .push(
                    widget::text_input("", state.threshold_text.as_str())
                        .on_input(Message::SleepThresholdTyped)
                        .width(Length::Fixed(70.0)),
                )
                .push(text::body(threshold_unit.trim()))
                .push(
                    widget::slider(
                        threshold_range,
                        display_threshold,
                        Message::SleepThresholdChanged,
                    )
                    .step(1.0)
                    // Fixed: the row shrinks to its contents, so Fill would be 0 wide
                    .width(Length::Fixed(220.0)),
                ),
        ));

    if state.config.animation_source.is_percentage() {
        behavior_section = behavior_section.add(settings::item(
            "Show % on Icon",
            toggler(state.config.show_percentage).on_toggle(Message::ShowPercentageToggled),
        ));
    }

    settings::view_column(vec![
        page_title.into(),
        text::caption(
            "The runner speeds up with the selected metric. Click the applet for details, middle-click for a system monitor.",
        )
        .into(),
        behavior_section.into(),
    ])
    .into()
}
