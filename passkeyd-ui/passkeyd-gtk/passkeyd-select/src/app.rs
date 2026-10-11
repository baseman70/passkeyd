//! Glue: owns the current `Screen`, applies core events and user actions to
//! it, and redraws. Mirrors `app.rs` of the iced front end.

use std::cell::RefCell;
use std::fmt::Display;
use std::process;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, glib};
use passkeyd_abi::utils::UIMessage;

use crate::ipc::{self, Event, Listener};
use crate::screen::{Flow, Screen};
use crate::ui::Ui;

/// Everything read from the core before the window exists.
pub struct Launch {
    pub screen: Screen,
    /// Sent once the window is up when there was nothing to choose from.
    pub first: Option<UIMessage>,
    pub listener: Listener,
}

struct App {
    ui: Ui,
    screen: RefCell<Screen>,
}

pub fn start(application: &Application, launch: Launch) {
    let app = Rc::new(App {
        ui: Ui::new(application),
        screen: RefCell::new(launch.screen),
    });
    app.wire();
    app.show();
    app.ui.window.present();

    if let Some(message) = launch.first {
        deliver(message);
    }

    // The reader thread can't touch widgets; events cross over a channel and
    // are handled on the main loop.
    let (tx, rx) = async_channel::unbounded();
    launch
        .listener
        .spawn(move |event| tx.send_blocking(event).is_ok());
    glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            app.on_event(event);
        }
    });
}

impl App {
    fn wire(self: &Rc<Self>) {
        self.ui.list.connect_row_activated({
            let app = self.clone();
            move |_, row| app.choose(row.index() as usize)
        });
        self.ui.entry.connect_activate({
            let app = self.clone();
            move |_| app.submit()
        });
        self.ui.authorize.connect_clicked({
            let app = self.clone();
            move |_| app.submit()
        });
        // The core notices the dead child and denies the operation.
        self.ui.cancel.connect_clicked(|_| process::exit(1));
        self.ui.window.connect_close_request(|_| process::exit(1));
    }

    fn show(&self) {
        self.ui.render(&self.screen.borrow().view());
    }

    fn on_event(&self, event: Event) {
        let flow = self.screen.borrow_mut().on_event(event);
        match flow {
            Flow::Continue => self.show(),
            // The core is done with us (success or failure), so just go away.
            Flow::Exit => process::exit(0),
            Flow::Fail(reason) => fail(reason),
        }
    }

    fn choose(&self, index: usize) {
        let message = self.screen.borrow_mut().choose(index);
        if let Some(message) = message {
            self.show();
            deliver(message);
        }
    }

    fn submit(&self) {
        let answer = self.ui.entry.text().to_string();
        let message = self.screen.borrow_mut().submit(answer);
        if let Some(message) = message {
            self.ui.entry.set_text("");
            self.show();
            deliver(message);
        }
    }
}

fn deliver(message: UIMessage) {
    if let Err(e) = ipc::send(message) {
        fail(e);
    }
}

pub fn fail(reason: impl Display) -> ! {
    eprintln!("passkeyd-gtk-select: {reason}");
    process::exit(1)
}