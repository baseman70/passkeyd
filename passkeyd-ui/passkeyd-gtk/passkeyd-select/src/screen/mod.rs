//! The flow of the window as plain Rust: which screen is showing and how
//! core events and user actions move it along. No toolkit in here, so it stays
//! easy to read and to test. `ui.rs` draws a [`View`] with GTK.

mod fingerprint;
mod form;
mod select;

use passkeyd_abi::utils::{SelectUI, UIMessage};
use passkeyd_locale::translate;
use zeroize::Zeroize;

pub use fingerprint::Fingerprint;
pub use form::Form;
pub use select::Select;

use crate::ipc::{Event, Method};

/// Which QML view is on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Select,
    Waiting,
    Form,
    Fingerprint,
}

impl Page {
    /// The name QML switches on.
    pub fn as_str(self) -> &'static str {
        match self {
            Page::Select => "select",
            Page::Waiting => "waiting",
            Page::Form => "form",
            Page::Fingerprint => "fingerprint",
        }
    }
}

/// A line under the description: a hint, or an error when `is_error`.
#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub is_error: bool,
}

/// The text field of a form.
#[derive(Debug, Clone)]
pub struct FieldView {
    pub secure: bool,
    pub placeholder: String,
    /// True while the core is checking the last answer.
    pub locked: bool,
}

/// Everything QML needs to draw the current screen.
#[derive(Debug, Clone)]
pub struct View {
    pub page: Page,
    pub title: String,
    pub description: String,
    pub notice: Option<Notice>,
    pub field: Option<FieldView>,
    pub accounts: Vec<String>,
}

impl View {
    fn authorize(page: Page) -> Self {
        View {
            page,
            title: authorize_title(),
            description: String::new(),
            notice: None,
            field: None,
            accounts: Vec::new(),
        }
    }
}

/// What the app should do after a core event.
#[derive(Debug)]
pub enum Flow {
    Continue,
    /// The core is done with us (success or failure), so just go away.
    Exit,
    Fail(String),
}

pub enum Screen {
    Select(Select),
    /// Choice sent; the core will either pick an auth method or close us.
    Waiting,
    Form(Form),
    Fingerprint(Fingerprint),
}

impl Screen {
    /// First screen for the state the core sent. A single account has nothing
    /// to choose from, so it is answered right away (second tuple item).
    pub fn boot(init: SelectUI) -> Result<(Self, Option<UIMessage>), &'static str> {
        match init.other_uis.len() {
            0 => Err("the core sent no passkeys to choose from"),
            1 => Ok((
                Screen::Waiting,
                Some(UIMessage::SelectionDoneMaybeStartAuth(0)),
            )),
            _ => Ok((Screen::Select(Select::new(init.rp, init.other_uis)), None)),
        }
    }

    /// Adding an authentication method: add a screen and one arm here.
    pub fn for_method(method: Method) -> Self {
        match method {
            Method::Password => Screen::Form(Form::password()),
            Method::Pam => Screen::Form(Form::pam()),
            Method::Fingerprint => Screen::Fingerprint(Fingerprint::default()),
        }
    }

    pub fn on_event(&mut self, event: Event) -> Flow {
        match event {
            Event::UseMethod(method) => *self = Screen::for_method(method),
            Event::Fallback(reason) => *self = Screen::Form(Form::fallback(reason)),
            Event::Retry => match self {
                Screen::Form(form) => form.retry(),
                Screen::Fingerprint(fingerprint) => fingerprint.retry(),
                _ => {}
            },
            Event::Pam(line) => {
                if let Screen::Form(form) = self {
                    form.pam_line(line);
                }
            }
            Event::Closed => return Flow::Exit,
            Event::Failed(reason) => return Flow::Fail(reason),
        }
        Flow::Continue
    }

    /// The user picked an account. The core decides what happens next.
    pub fn choose(&mut self, index: usize) -> Option<UIMessage> {
        let valid = matches!(self, Screen::Select(select) if index < select.len());
        if !valid {
            return None;
        }
        *self = Screen::Waiting;
        Some(UIMessage::SelectionDoneMaybeStartAuth(index))
    }

    /// The user submitted the text field. `None` when there is nothing to answer.
    pub fn submit(&mut self, mut answer: String) -> Option<UIMessage> {
        match self {
            Screen::Form(form) => form.submit(answer),
            _ => {
                answer.zeroize();
                None
            }
        }
    }

    pub fn view(&self) -> View {
        match self {
            Screen::Select(screen) => screen.view(),
            Screen::Form(screen) => screen.view(),
            Screen::Fingerprint(screen) => screen.view(),
            Screen::Waiting => View::authorize(Page::Waiting),
        }
    }
}

fn authorize_title() -> String {
    translate!("ui.select.authorize.title_bar").to_string()
}