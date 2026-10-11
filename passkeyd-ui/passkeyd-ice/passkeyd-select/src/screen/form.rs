use iced::widget::{column, container, text, text_input};
use iced::{Alignment, Length, Padding};
use passkeyd_abi::component::title_bar_component;
use passkeyd_abi::theme::TextClass;
use passkeyd_abi::utils::{FallbackToPasswordReason, UIMessage};
use passkeyd_locale::translate;

use super::{Element, frame};
use crate::app::User;
use crate::ipc::PamLine;

/// Which `UIMessage` an answer is sent as.
#[derive(Clone, Copy)]
enum Answer {
    Password,
    Pam,
}

struct Notice {
    text: String,
    is_error: bool,
}

struct Field {
    value: String,
    secure: bool,
    placeholder: String,
    /// True while the core is checking the last answer.
    locked: bool,
}

impl Field {
    fn new(secure: bool) -> Self {
        let placeholder = if secure {
            translate!("ui.select.authorize.password").to_string()
        } else {
            String::new()
        };
        Self {
            value: String::new(),
            secure,
            placeholder,
            locked: false,
        }
    }
}

/// A single text prompt: the password form, or one step of a PAM conversation.
pub struct Form {
    answer: Answer,
    description: String,
    notice: Option<Notice>,
    /// `None` while there is nothing to type yet (PAM hasn't asked anything).
    field: Option<Field>,
}

impl Form {
    pub fn password() -> Self {
        Self {
            answer: Answer::Password,
            description: translate!("ui.select.authorize.description").to_string(),
            notice: None,
            field: Some(Field::new(true)),
        }
    }

    /// Password form shown because another method gave up.
    pub fn fallback(reason: FallbackToPasswordReason) -> Self {
        let text = match reason {
            FallbackToPasswordReason::FingerprintDeviceUnavailable => {
                translate!("ui.select.authorize.fallback_fprint_device")
            }
            FallbackToPasswordReason::FingerprintUserUnavailable => {
                translate!("ui.select.authorize.fallback_fprint_user")
            }
            FallbackToPasswordReason::NoEnrolledFingerprints => {
                translate!("ui.select.authorize.fallback_no_fprints")
            }
            FallbackToPasswordReason::PamUnavailable => {
                translate!("ui.select.authorize.fallback_pam")
            }
        }
        .to_string();

        let mut form = Self::password();
        form.notice = Some(Notice {
            text,
            is_error: false,
        });
        form
    }

    pub fn pam() -> Self {
        Self {
            answer: Answer::Pam,
            description: translate!("ui.select.authorize.pam_description").to_string(),
            notice: None,
            field: None,
        }
    }

    /// The core rejected the last answer.
    pub fn retry(&mut self) {
        self.notice = Some(Notice {
            text: translate!("ui.select.authorize.helper_text").to_string(),
            is_error: true,
        });
        if let Some(field) = &mut self.field {
            field.locked = false;
        }
    }

    pub fn pam_line(&mut self, line: PamLine) {
        match line {
            PamLine::Info(text) => {
                self.notice = Some(Notice {
                    text,
                    is_error: false,
                })
            }
            PamLine::Error(text) => {
                self.notice = Some(Notice {
                    text,
                    is_error: true,
                })
            }
            PamLine::Ask { prompt, secure } => {
                self.description = prompt;
                self.field = Some(Field::new(secure));
            }
        }
    }

    pub fn edit(&mut self, value: String) {
        if let Some(field) = &mut self.field {
            field.value = value;
        }
    }

    /// Returns what to send to the core, if there is an answer to give.
    pub fn submit(&mut self) -> Option<UIMessage> {
        let field = self.field.as_mut().filter(|field| !field.locked)?;
        field.locked = true;
        let answer = std::mem::take(&mut field.value);
        self.notice = None;

        Some(match self.answer {
            Answer::Password => UIMessage::Password(answer),
            Answer::Pam => UIMessage::PAMAnswer(answer),
        })
    }

    pub fn view(&self) -> Element<'_> {
        let title_bar: Element<'_> =
            title_bar_component(translate!("ui.select.authorize.title_bar"), User::Cancel).into();

        let description = text(self.description.as_str())
            .class(TextClass::SecondaryText)
            .size(16);

        let input = self.field.as_ref().map(|field| {
            let input = text_input(&field.placeholder, &field.value)
                .secure(field.secure)
                .padding(Padding::new(12.0))
                .width(354);
            // Without `on_input` the widget is disabled.
            if field.locked {
                input
            } else {
                input.on_input(User::Edit).on_submit(User::Submit)
            }
        });

        let notice = self.notice.as_ref().map(|notice| {
            text(notice.text.as_str()).class(if notice.is_error {
                TextClass::ErrorText
            } else {
                TextClass::SecondaryText
            })
        });

        let controls: Element<'_> = container(column![input, notice])
            .height(Length::Fixed(110.))
            .width(Length::Fill)
            .align_y(Alignment::Center)
            .align_x(Alignment::Center)
            .into();

        let body: Element<'_> = column![description, controls].height(Length::Fill).into();
        frame(title_bar, body)
    }
}
