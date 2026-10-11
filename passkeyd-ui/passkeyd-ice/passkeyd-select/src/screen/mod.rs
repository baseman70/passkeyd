mod fingerprint;
mod form;
mod select;

use iced::Length;
use iced::widget::{Space, column};
use passkeyd_abi::component::title_bar_component;
use passkeyd_abi::theme::StylisedTheme;
use passkeyd_locale::translate;

pub use fingerprint::Fingerprint;
pub use form::Form;
pub use select::Select;

use crate::app::User;
use crate::ipc::Method;

pub type Element<'a> = iced::Element<'a, User, StylisedTheme>;

pub enum Screen {
    Select(Select),
    /// Choice sent; the core will either pick an auth method or close us.
    Waiting,
    Form(Form),
    Fingerprint(Fingerprint),
}

impl Screen {
    /// Adding an authentication method: add a screen and one arm here.
    pub fn for_method(method: Method) -> Self {
        match method {
            Method::Password => Screen::Form(Form::password()),
            Method::Pam => Screen::Form(Form::pam()),
            Method::Fingerprint => Screen::Fingerprint(Fingerprint::default()),
        }
    }

    pub fn view(&self) -> Element<'_> {
        match self {
            Screen::Select(screen) => screen.view(),
            Screen::Form(screen) => screen.view(),
            Screen::Fingerprint(screen) => screen.view(),
            Screen::Waiting => {
                let title_bar: Element<'_> =
                    title_bar_component(translate!("ui.select.authorize.title_bar"), User::Cancel)
                        .into();
                frame(title_bar, Space::new().into())
            }
        }
    }
}

/// Shared window chrome: title bar on top, body below.
pub fn frame<'a>(title_bar: Element<'a>, body: Element<'a>) -> Element<'a> {
    column![title_bar, body]
        .spacing(16)
        .padding([26, 26])
        .height(Length::Fill)
        .into()
}