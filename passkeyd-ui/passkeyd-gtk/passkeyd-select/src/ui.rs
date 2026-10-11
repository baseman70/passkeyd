//! The GTK widgets. Pure drawing: `render` turns a `View` into widget state,
//! and the widgets that produce user input are public so `app.rs` can wire them.

use std::cell::Cell;

use gtk4::pango;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, HeaderBar, Image,
    InputPurpose, Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow,
    SelectionMode, Spinner, Stack, StackTransitionType,
};

use crate::screen::{Notice, Page, View};

pub struct Ui {
    pub window: ApplicationWindow,
    stack: Stack,

    select_description: Label,
    pub list: ListBox,
    /// How many rows `list` currently has.
    rows: Cell<usize>,

    form_description: Label,
    pub entry: Entry,
    form_notice: Label,
    pub cancel: Button,
    pub authorize: Button,
    /// (page, has field, locked) at the last render, to grab focus only on a change.
    focus_state: Cell<(Page, bool, bool)>,

    fingerprint_description: Label,
    fingerprint_notice: Label,
}

impl Ui {
    pub fn new(application: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(application)
            .default_width(420)
            .default_height(230)
            .resizable(false)
            .build();
        window.set_titlebar(Some(&HeaderBar::builder().show_title_buttons(true).build()));

        let stack = Stack::new();
        stack.set_transition_type(StackTransitionType::SlideLeftRight);

        // Select
        let select = page();
        let select_description = description();
        let list = ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(SelectionMode::None);
        select.append(&select_description);
        select.append(
            &ScrolledWindow::builder()
                .hscrollbar_policy(PolicyType::Never)
                .vscrollbar_policy(PolicyType::Automatic)
                .min_content_height(100)
                .max_content_height(140)
                .child(&list)
                .build(),
        );

        // Waiting
        let waiting = page();
        let spinner = Spinner::builder()
            .spinning(true)
            .halign(Align::Center)
            .valign(Align::Center)
            .vexpand(true)
            .width_request(32)
            .height_request(32)
            .build();
        waiting.append(&spinner);

        // Form
        let form = page();
        let form_description = description();
        let entry = Entry::builder().hexpand(true).build();
        let form_notice = notice_label();
        let cancel = Button::builder().label("Cancel").build();
        let authorize = Button::builder().label("Authorize").build();
        authorize.add_css_class("suggested-action");
        let buttons = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .halign(Align::End)
            .valign(Align::End)
            .spacing(8)
            .build();
        buttons.append(&cancel);
        buttons.append(&authorize);
        form.append(&form_description);
        form.append(&entry);
        form.append(&form_notice);
        form.append(&GtkBox::builder().vexpand(true).build());
        form.append(&buttons);

        // Fingerprint
        let fingerprint = page();
        let icon = Image::from_icon_name("fingerprint-symbolic");
        icon.set_pixel_size(48);
        icon.set_valign(Align::End);
        icon.set_vexpand(true);
        let fingerprint_description = description();
        fingerprint_description.set_xalign(0.5);
        fingerprint_description.set_justify(gtk4::Justification::Center);
        let fingerprint_notice = notice_label();
        fingerprint_notice.set_xalign(0.5);
        fingerprint_notice.set_justify(gtk4::Justification::Center);
        fingerprint.append(&icon);
        fingerprint.append(&fingerprint_description);
        fingerprint.append(&fingerprint_notice);
        fingerprint.append(&GtkBox::builder().vexpand(true).build());

        stack.add_named(&select, Some(Page::Select.as_str()));
        stack.add_named(&waiting, Some(Page::Waiting.as_str()));
        stack.add_named(&form, Some(Page::Form.as_str()));
        stack.add_named(&fingerprint, Some(Page::Fingerprint.as_str()));
        window.set_child(Some(&stack));

        Self {
            window,
            stack,
            select_description,
            list,
            rows: Cell::new(0),
            form_description,
            entry,
            form_notice,
            cancel,
            authorize,
            focus_state: Cell::new((Page::Waiting, false, true)),
            fingerprint_description,
            fingerprint_notice,
        }
    }

    pub fn render(&self, view: &View) {
        self.window.set_title(Some(&view.title));
        self.stack.set_visible_child_name(view.page.as_str());

        match view.page {
            Page::Select => {
                self.select_description.set_label(&view.description);
                self.fill_accounts(&view.accounts);
            }
            Page::Waiting => {}
            Page::Form => {
                self.form_description.set_label(&view.description);
                set_notice(&self.form_notice, view.notice.as_ref());
                self.render_field(view);
            }
            Page::Fingerprint => {
                self.fingerprint_description.set_label(&view.description);
                set_notice(&self.fingerprint_notice, view.notice.as_ref());
            }
        }

        let (has_field, locked) = view
            .field
            .as_ref()
            .map_or((false, true), |field| (true, field.locked));
        let state = (view.page, has_field, locked);
        // Focus only when the field becomes usable: grabbing on every render
        // would select what the user is typing whenever a PAM info line arrives.
        if state != self.focus_state.replace(state) && view.page == Page::Form && !locked {
            self.entry.grab_focus();
        }
    }

    fn render_field(&self, view: &View) {
        let Some(field) = &view.field else {
            self.entry.set_visible(false);
            self.authorize.set_sensitive(false);
            return;
        };
        self.entry.set_visible(true);
        self.entry.set_visibility(!field.secure);
        self.entry.set_input_purpose(if field.secure {
            InputPurpose::Password
        } else {
            InputPurpose::FreeForm
        });
        self.entry.set_placeholder_text(Some(&field.placeholder));
        // Locked while the core checks the last answer.
        self.entry.set_sensitive(!field.locked);
        self.authorize.set_sensitive(!field.locked);
    }

    /// The account list never changes after boot; build it once.
    fn fill_accounts(&self, accounts: &[String]) {
        if self.rows.get() == accounts.len() {
            return;
        }
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        for account in accounts {
            self.list.append(&account_row(account));
        }
        self.rows.set(accounts.len());
    }
}

fn account_row(label: &str) -> ListBoxRow {
    let content = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(10)
        .margin_bottom(10)
        .build();

    let icon = Image::from_icon_name("avatar-default-symbolic");
    icon.set_pixel_size(16);
    content.append(&icon);
    content.append(
        &Label::builder()
            .label(label)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(pango::EllipsizeMode::End)
            .build(),
    );

    let row = ListBoxRow::new();
    row.set_cursor_from_name(Some("pointer"));
    row.set_child(Some(&content));
    row
}

fn page() -> GtkBox {
    GtkBox::builder()
        .orientation(Orientation::Vertical)
        .margin_start(16)
        .margin_end(16)
        .margin_top(16)
        .margin_bottom(16)
        .spacing(12)
        .build()
}

fn description() -> Label {
    Label::builder().wrap(true).xalign(0.0).build()
}

fn notice_label() -> Label {
    Label::builder().wrap(true).xalign(0.0).visible(false).build()
}

/// A hint is dimmed; an error uses the theme's error style.
fn set_notice(label: &Label, notice: Option<&Notice>) {
    label.remove_css_class("error");
    label.remove_css_class("dim-label");
    match notice {
        Some(notice) => {
            label.set_label(&notice.text);
            label.add_css_class(if notice.is_error { "error" } else { "dim-label" });
            label.set_visible(true);
        }
        None => label.set_visible(false),
    }
}