use passkeyd_locale::translate;

use super::{Notice, Page, View, authorize_title};

/// Nothing to type: the core drives the sensor and tells us when to retry.
#[derive(Default)]
pub struct Fingerprint {
    failed: bool,
}

impl Fingerprint {
    pub fn retry(&mut self) {
        self.failed = true;
    }

    pub fn view(&self) -> View {
        View {
            page: Page::Fingerprint,
            title: authorize_title(),
            description: translate!("ui.select.authorize.fingerprint_description").to_string(),
            notice: self.failed.then(|| Notice {
                text: translate!("ui.select.authorize.fingerprint_retry").to_string(),
                is_error: true,
            }),
            field: None,
            accounts: Vec::new(),
        }
    }
}
