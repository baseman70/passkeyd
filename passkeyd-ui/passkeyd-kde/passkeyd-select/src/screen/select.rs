use ctap_types::webauthn::PublicKeyCredentialRpEntity;
use passkeyd_abi::database::layout::OtherUI;
use passkeyd_locale::translate;

use super::{Page, View};

/// Account picker: one row per passkey the core found for the site.
pub struct Select {
    rp_id: String,
    labels: Vec<String>,
}

impl Select {
    pub fn new(rp: PublicKeyCredentialRpEntity, accounts: Vec<OtherUI>) -> Self {
        Self {
            rp_id: rp.id.as_str().to_owned(),
            labels: accounts.iter().map(label).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }

    pub fn view(&self) -> View {
        View {
            page: Page::Select,
            title: translate!("ui.select.select.title_bar").to_string(),
            description: translate!(
                "ui.select.select.description",
                site => self.rp_id.as_str()
            )
            .to_string(),
            notice: None,
            field: None,
            accounts: self.labels.clone(),
        }
    }
}

/// What a row shows: the display name, else the name, else the raw user id.
fn label(account: &OtherUI) -> String {
    let user = &account.user;
    user.display_name
        .as_ref()
        .or(user.name.as_ref())
        .map(|name| name.as_str().to_owned())
        .unwrap_or_else(|| String::from_utf8_lossy(user.id.as_slice()).into_owned())
}