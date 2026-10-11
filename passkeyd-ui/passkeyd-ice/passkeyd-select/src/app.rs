use std::fmt::Display;
use std::process;

use iced::Task;
use iced::widget::operation::focus_next;
use passkeyd_abi::theme::StylisedTheme;
use passkeyd_abi::utils::UIMessage;

use crate::ipc::{self, Event};
use crate::screen::{Form, Screen, Select};

#[derive(Debug)]
pub enum Message {
    /// Something the core told us.
    Core(Event),
    /// Something the user did.
    User(User),
}

/// What the user can do. Screens only ever produce these.
#[derive(Debug, Clone)]
pub enum User {
    Select(usize),
    Edit(String),
    Submit,
    Cancel,
}

pub struct App {
    screen: Screen,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let (init, events) = ipc::connect().unwrap_or_else(|e| fail(e));
        let listen = Task::run(events, Message::Core);

        let mut app = App { screen: Screen::Waiting };
        let first = if init.other_uis.len() == 1 {
            app.choose(0) // nothing to choose from
        } else {
            app.screen = Screen::Select(Select::new(init.rp, init.other_uis));
            Task::none()
        };
        (app, Task::batch([listen, first]))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Core(event) => self.on_core(event),
            Message::User(user) => self.on_user(user),
        }
    }

    pub fn view(&self) -> iced::Element<'_, Message, StylisedTheme> {
        self.screen.view().map(Message::User)
    }

    fn on_core(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::UseMethod(method) => self.screen = Screen::for_method(method),
            Event::Fallback(reason) => self.screen = Screen::Form(Form::fallback(reason)),
            Event::Retry => match &mut self.screen {
                Screen::Form(form) => form.retry(),
                Screen::Fingerprint(fingerprint) => fingerprint.retry(),
                _ => {}
            },
            Event::Pam(line) => {
                if let Screen::Form(form) = &mut self.screen {
                    form.pam_line(line);
                }
            }
            // The core is done with us (success or failure), so just go away.
            Event::Closed => return iced::exit(),
            Event::Failed(reason) => fail(reason),
        }
        focus_next()
    }

    fn on_user(&mut self, user: User) -> Task<Message> {
        match user {
            // The core notices the dead child and denies the operation.
            User::Cancel => process::exit(1),
            User::Select(index) if matches!(self.screen, Screen::Select(_)) => self.choose(index),
            User::Select(_) => Task::none(),
            User::Edit(value) => {
                if let Screen::Form(form) = &mut self.screen {
                    form.edit(value);
                }
                Task::none()
            }
            User::Submit => match &mut self.screen {
                Screen::Form(form) => form.submit().map_or_else(Task::none, send),
                _ => Task::none(),
            },
        }
    }

    fn choose(&mut self, index: usize) -> Task<Message> {
        // The core now decides what happens next: authenticate or close us.
        self.screen = Screen::Waiting;
        send(UIMessage::SelectionDoneMaybeStartAuth(index))
    }
}

fn send(message: UIMessage) -> Task<Message> {
    match ipc::send(message) {
        Ok(()) => Task::none(),
        Err(e) => fail(e),
    }
}

fn fail(reason: impl Display) -> ! {
    eprintln!("passkeyd-ice-select: {reason}");
    process::exit(1)
}