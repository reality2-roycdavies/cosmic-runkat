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
use cosmic::widget::{self, container};
use cosmic::{Action, Application, Element, Task};

use crate::settings_page;

const APP_ID: &str = "io.github.reality2_roycdavies.cosmic-runkat.settings";

pub struct SettingsApp {
    core: Core,
    page: settings_page::State,
}

impl Application for SettingsApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = settings_page::Message;

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
        let page = settings_page::init();
        (Self { core, page }, Task::none())
    }

    fn update(&mut self, message: Self::Message) -> Task<Action<Self::Message>> {
        settings_page::update(&mut self.page, message);
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let content = settings_page::view(&self.page);

        widget::container(container(content).max_width(800))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .padding(24)
            .into()
    }
}

pub fn run_settings() -> cosmic::iced::Result {
    let settings = cosmic::app::Settings::default().size(cosmic::iced::Size::new(850.0, 380.0));
    cosmic::app::run::<SettingsApp>(settings, ())
}
