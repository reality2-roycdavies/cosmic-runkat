//! Embeddable settings page for cosmic-runkat
//!
//! Provides the settings UI as standalone State/Message/init/update/view
//! functions that can be embedded in cosmic-applet-settings or wrapped
//! in a standalone Application window.

use cosmic::iced::Length;
use cosmic::widget::{self, settings, text, toggler};
use cosmic::Element;

use crate::config::{AnimationSource, Config};

pub struct State {
    pub config: Config,
    pub max_freq_mhz: f32,
}

#[derive(Debug, Clone)]
pub enum Message {
    SleepThresholdChanged(f32),
    ShowPercentageToggled(bool),
    AnimationSourceChanged(AnimationSource),
}

pub fn init() -> State {
    let config = Config::load();
    let freq_info = crate::sysinfo::CpuFrequency::read();
    let max_freq_mhz = freq_info.max_per_core.first().copied().unwrap_or(5000) as f32;

    State { config, max_freq_mhz }
}

pub fn update(state: &mut State, message: Message) {
    match message {
        Message::SleepThresholdChanged(value) => {
            state.config.set_current_threshold(value);
            let _ = state.config.save();
        }
        Message::ShowPercentageToggled(value) => {
            state.config.show_percentage = value;
            let _ = state.config.save();
        }
        Message::AnimationSourceChanged(source) => {
            state.config.animation_source = source;
            let _ = state.config.save();
        }
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
            AnimationSource::CpuUsage => ("Sleep Below", 0.0..=30.0, "%"),
            AnimationSource::Frequency => ("Sleep Below", 0.0..=state.max_freq_mhz, " MHz"),
            AnimationSource::Temperature => ("Sleep Below", 20.0..=100.0, "\u{00b0}C"),
        };

    let display_threshold = state
        .config
        .current_threshold()
        .clamp(*threshold_range.start(), *threshold_range.end());

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

    if state.config.animation_source == AnimationSource::CpuUsage {
        behavior_section = behavior_section.add(settings::item(
            "Show % on Icon",
            toggler(state.config.show_percentage).on_toggle(Message::ShowPercentageToggled),
        ));
    }

    settings::view_column(vec![
        page_title.into(),
        text::caption(
            "The cat runs faster based on the selected metric. Click the panel applet to see details.",
        )
        .into(),
        behavior_section.into(),
    ])
    .into()
}
