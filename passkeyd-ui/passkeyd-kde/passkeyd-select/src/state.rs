//! The bridge between the plain-Rust flow (`screen`, `ipc`) and QML.
//!
//! `SelectionState` is the single QObject the QML window binds to. It owns the
//! current `Screen`, flattens its `View` into properties, turns user actions
//! (`choose`, `submit`, `cancel`) into messages for the core, and applies core
//! events that the reader thread queues onto the Qt thread.

use std::cell::RefCell;
use std::fmt::Display;
use std::pin::Pin;
use std::process;
use std::sync::Mutex;

use cxx_qt::Threading;
use cxx_qt_lib::{QString, QStringList};
use passkeyd_abi::utils::{SelectUI, UIMessage};

use crate::ipc::{self, Event, Listener};
use crate::screen::{Flow, Screen};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// "select", "waiting", "form" or "fingerprint".
        #[qproperty(QString, page)]
        #[qproperty(QString, title)]
        #[qproperty(QString, description)]
        #[qproperty(QString, notice)]
        #[qproperty(bool, notice_is_error)]
        #[qproperty(QStringList, accounts)]
        #[qproperty(bool, has_field)]
        #[qproperty(bool, field_secure)]
        #[qproperty(QString, field_placeholder)]
        #[qproperty(bool, field_locked)]
        type SelectionState = super::SelectionStateRust;

        /// Call once when the window is ready: shows the first screen and
        /// starts listening to the core.
        #[qinvokable]
        fn start(self: Pin<&mut SelectionState>);

        #[qinvokable]
        fn choose(self: Pin<&mut SelectionState>, index: u32);

        #[qinvokable]
        fn submit(self: Pin<&mut SelectionState>, answer: QString);

        #[qinvokable]
        fn cancel(self: &SelectionState);
    }

    impl cxx_qt::Threading for SelectionState {}
}

/// Initial state and pipe, read in `main` before Qt starts and picked up
/// when QML creates the object.
struct Handoff {
    init: SelectUI,
    listener: Listener,
}

static HANDOFF: Mutex<Option<Handoff>> = Mutex::new(None);

pub fn hand_off(init: SelectUI, listener: Listener) {
    *HANDOFF.lock().unwrap_or_else(|e| e.into_inner()) = Some(Handoff { init, listener });
}

pub struct SelectionStateRust {
    page: QString,
    title: QString,
    description: QString,
    notice: QString,
    notice_is_error: bool,
    accounts: QStringList,
    has_field: bool,
    field_secure: bool,
    field_placeholder: QString,
    field_locked: bool,

    // Not exposed to QML. Interior mutability because Qt hands us `Pin<&mut _>`.
    screen: RefCell<Screen>,
    /// Sent on `start` when there was nothing to choose from.
    first: RefCell<Option<UIMessage>>,
    listener: RefCell<Option<Listener>>,
}

impl Default for SelectionStateRust {
    fn default() -> Self {
        let handoff = HANDOFF.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(Handoff { init, listener }) = handoff else {
            fail("the selection state was created before the core connected, or twice")
        };
        let (screen, first) = Screen::boot(init).unwrap_or_else(|reason| fail(reason));

        Self {
            page: QString::default(),
            title: QString::default(),
            description: QString::default(),
            notice: QString::default(),
            notice_is_error: false,
            accounts: QStringList::default(),
            has_field: false,
            field_secure: false,
            field_placeholder: QString::default(),
            field_locked: false,
            screen: RefCell::new(screen),
            first: RefCell::new(first),
            listener: RefCell::new(Some(listener)),
        }
    }
}

impl qobject::SelectionState {
    pub fn start(mut self: Pin<&mut Self>) {
        self.as_mut().show();

        let first = self.first.take();
        if let Some(message) = first {
            deliver(message);
        }

        let listener = self.listener.take();
        if let Some(listener) = listener {
            let qt = self.qt_thread();
            listener.spawn(move |event| qt.queue(move |state| state.on_event(event)).is_ok());
        }
    }

    pub fn choose(mut self: Pin<&mut Self>, index: u32) {
        let message = self.screen.borrow_mut().choose(index as usize);
        if let Some(message) = message {
            self.as_mut().show();
            deliver(message);
        }
    }

    pub fn submit(mut self: Pin<&mut Self>, answer: QString) {
        let message = self.screen.borrow_mut().submit(answer.to_string());
        if let Some(message) = message {
            self.as_mut().show();
            deliver(message);
        }
    }

    /// The core notices the dead child and denies the operation.
    pub fn cancel(&self) {
        process::exit(1)
    }

    /// Runs on the Qt thread, queued by the reader thread.
    fn on_event(self: Pin<&mut Self>, event: Event) {
        let flow = self.screen.borrow_mut().on_event(event);
        match flow {
            Flow::Continue => self.show(),
            // The core is done with us (success or failure), so just go away.
            Flow::Exit => process::exit(0),
            Flow::Fail(reason) => fail(reason),
        }
    }

    /// Copies the current screen into the QML properties.
    fn show(mut self: Pin<&mut Self>) {
        let view = self.screen.borrow().view();

        self.as_mut().set_page(QString::from(view.page.as_str()));
        self.as_mut().set_title(QString::from(view.title.as_str()));
        self.as_mut()
            .set_description(QString::from(view.description.as_str()));

        let (notice, is_error) = view.notice.as_ref().map_or(("", false), |notice| {
            (notice.text.as_str(), notice.is_error)
        });
        self.as_mut().set_notice(QString::from(notice));
        self.as_mut().set_notice_is_error(is_error);

        let mut accounts = QStringList::default();
        for account in &view.accounts {
            accounts.append(QString::from(account.as_str()));
        }
        self.as_mut().set_accounts(accounts);

        let field = view.field.as_ref();
        self.as_mut().set_has_field(field.is_some());
        self.as_mut()
            .set_field_secure(field.is_some_and(|field| field.secure));
        self.as_mut().set_field_placeholder(QString::from(
            field.map_or("", |field| field.placeholder.as_str()),
        ));
        self.as_mut()
            .set_field_locked(field.is_some_and(|field| field.locked));
    }
}

fn deliver(message: UIMessage) {
    if let Err(e) = ipc::send(message) {
        fail(e);
    }
}

pub fn fail(reason: impl Display) -> ! {
    eprintln!("passkeyd-kde-select: {reason}");
    process::exit(1)
}
