use passkeyd_abi::utils::{FallbackToPasswordReason, UIMessage};
use passkeyd_locale::translate;
use zeroize::Zeroize;

use super::{FieldView, Notice, Page, View, authorize_title};
use crate::ipc::PamLine;

/// Which `UIMessage` an answer is sent as.
#[derive(Clone, Copy)]
enum Answer {
    Password,
    Pam,
}

struct Field {
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
            secure,
            placeholder,
            locked: false,
        }
    }
}

/// A single text prompt: the password form, or one step of a PAM conversation.
/// The typed text lives in the QML field and arrives with `submit`.
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

    /// Returns what to send to the core, if there is an answer to give.
    /// The field stays locked until the core answers (`retry` or a new prompt).
    pub fn submit(&mut self, mut answer: String) -> Option<UIMessage> {
        let Some(field) = self.field.as_mut().filter(|field| !field.locked) else {
            answer.zeroize();
            return None;
        };
        field.locked = true;
        self.notice = None;

        Some(match self.answer {
            Answer::Password => UIMessage::Password(answer),
            Answer::Pam => UIMessage::PAMAnswer(answer),
        })
    }

    pub fn view(&self) -> View {
        View {
            page: Page::Form,
            title: authorize_title(),
            description: self.description.clone(),
            notice: self.notice.clone(),
            field: self.field.as_ref().map(|field| FieldView {
                secure: field.secure,
                placeholder: field.placeholder.clone(),
                locked: field.locked,
            }),
            accounts: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    fn init() {
        static ONCE: Once = Once::new();
        ONCE.call_once(passkeyd_locale::init_translations);
    }

    #[test]
    fn password_locks_until_retry() {
        init();
        let mut form = Form::password();
        assert!(matches!(
            form.submit("hunter2".into()),
            Some(UIMessage::Password(_))
        ));
        assert!(form.submit("again".into()).is_none(), "locked");
        form.retry();
        assert!(form.view().notice.is_some_and(|n| n.is_error));
        assert!(form.submit("again".into()).is_some(), "unlocked by retry");
    }

    #[test]
    fn pam_has_no_field_until_asked() {
        init();
        let mut form = Form::pam();
        assert!(form.submit("x".into()).is_none());
        form.pam_line(PamLine::Ask {
            prompt: "Code:".into(),
            secure: false,
        });
        let view = form.view();
        assert_eq!(view.description, "Code:");
        assert!(view.field.is_some_and(|f| !f.secure && !f.locked));
        assert!(matches!(
            form.submit("123".into()),
            Some(UIMessage::PAMAnswer(_))
        ));
    }
}