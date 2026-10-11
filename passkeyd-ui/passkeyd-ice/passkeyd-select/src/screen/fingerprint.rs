use iced::widget::{column, container, text};
use iced::{Alignment, Length};
use passkeyd_abi::component::title_bar_component;
use passkeyd_abi::theme::TextClass;
use passkeyd_locale::translate;

use super::{Element, frame};
use crate::app::User;

/// Nothing to type: the core drives the sensor and tells us when to retry.
#[derive(Default)]
pub struct Fingerprint {
    failed: bool,
}

impl Fingerprint {
    pub fn retry(&mut self) {
        self.failed = true;
    }

    pub fn view(&self) -> Element<'_> {
        let title_bar: Element<'_> =
            title_bar_component(translate!("ui.select.authorize.title_bar"), User::Cancel).into();

        let description = text(translate!("ui.select.authorize.fingerprint_description"))
            .class(TextClass::SecondaryText)
            .size(16);

        let status = self.failed.then(|| {
            text(translate!("ui.select.authorize.fingerprint_retry")).class(TextClass::ErrorText)
        });

        let body: Element<'_> = container(
            column![description, status]
                .spacing(8)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into();

        frame(title_bar, body)
    }
}
