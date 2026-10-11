use ctap_types::webauthn::PublicKeyCredentialRpEntity;
use iced::widget::scrollable::Scrollbar;
use iced::widget::{Column, column, container, scrollable, text};
use iced::{Alignment, Length, Padding};
use passkeyd_abi::component::{title_bar_component, user_component};
use passkeyd_abi::database::layout::OtherUI;
use passkeyd_abi::theme::TextClass;
use passkeyd_locale::translate;

use super::{Element, frame};
use crate::app::User;

pub struct Select {
    rp: PublicKeyCredentialRpEntity,
    accounts: Vec<OtherUI>,
}

impl Select {
    pub fn new(rp: PublicKeyCredentialRpEntity, accounts: Vec<OtherUI>) -> Self {
        Self { rp, accounts }
    }

    pub fn view(&self) -> Element<'_> {
        let title_bar: Element<'_> =
            title_bar_component(translate!("ui.select.select.title_bar"), User::Cancel).into();

        let description = text(translate!(
            "ui.select.select.description",
            site => self.rp.id.as_str()
        ))
        .class(TextClass::SecondaryText)
        .height(Length::Shrink)
        .width(Length::Fill);

        let accounts = self
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                user_component(&self.rp, account, Some(User::Select(index))).into()
            })
            .collect::<Column<'_, User, _>>()
            .spacing(4)
            .padding(Padding { top: 5.0, ..Default::default() });

        let list = container(
            scrollable(
                container(accounts)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center)
                    .width(Length::Fill),
            )
            .direction(scrollable::Direction::Vertical(Scrollbar::new()))
            .height(Length::Fill)
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .padding(Padding { right: 24., ..Default::default() });

        let body: Element<'_> = column![description, list]
            .spacing(18)
            .height(Length::Fill)
            .into();

        frame(title_bar, body)
    }
}