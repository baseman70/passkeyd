//! The only module that touches the pipes to the core.
//!
//! Both directions carry back-to-back CBOR items:
//!   core -> ui (stdin):  one `SelectUI`, then any number of `ServiceMessage`s
//!   ui -> core (stdout): any number of `UIMessage`s (no prefix byte)
//! The core closing stdin means "you're done". Never print to stdout elsewhere.

use std::io::{self, Read, Write};
use std::thread;

use ctap_types::serde::cbor_deserialize;
use iced::futures::channel::mpsc;
use passkeyd_abi::config::Auth;
use passkeyd_abi::utils::{
    CborVec, ConversationQuestion, FallbackToPasswordReason, SelectUI, ServiceMessage, UIMessage,
};
use zeroize::Zeroize;

/// Core messages are tiny; anything bigger than this is a protocol error.
const MAX_BUFFERED: usize = 16 * 1024;

/// How the user has to authenticate. One variant per `Auth` in the core.
#[derive(Debug, Clone, Copy)]
pub enum Method {
    Password,
    Fingerprint,
    Pam,
}

/// One line of a PAM conversation.
#[derive(Debug)]
pub enum PamLine {
    Info(String),
    Error(String),
    Ask { prompt: String, secure: bool },
}

/// What the core can tell the GUI.
#[derive(Debug)]
pub enum Event {
    UseMethod(Method),
    Fallback(FallbackToPasswordReason),
    Retry,
    Pam(PamLine),
    /// The core closed the pipe.
    Closed,
    Failed(String),
}

impl From<ServiceMessage> for Event {
    fn from(message: ServiceMessage) -> Self {
        match message {
            ServiceMessage::AuthSelected(auth) => Event::UseMethod(match auth {
                Auth::PASS => Method::Password,
                Auth::FPRINT => Method::Fingerprint,
                Auth::PAM => Method::Pam,
            }),
            ServiceMessage::FallbackToPassword(reason) => Event::Fallback(reason),
            ServiceMessage::Retry => Event::Retry,
            ServiceMessage::PAMQuestion(question) => Event::Pam(match question {
                ConversationQuestion::Info(text) => PamLine::Info(text),
                ConversationQuestion::Error(text) => PamLine::Error(text),
                ConversationQuestion::SensitiveInput(prompt) => PamLine::Ask {
                    prompt,
                    secure: true,
                },
                ConversationQuestion::Input(prompt) => PamLine::Ask {
                    prompt,
                    secure: false,
                },
            }),
        }
    }
}

/// Reads the initial state, then forwards everything else as `Event`s.
pub fn connect() -> io::Result<(SelectUI, mpsc::UnboundedReceiver<Event>)> {
    let mut incoming = Incoming {
        stdin: io::stdin(),
        buf: Vec::new(),
    };
    let init = incoming.init()?;

    let (tx, rx) = mpsc::unbounded();
    thread::spawn(move || {
        loop {
            let event = match incoming.message() {
                Ok(Some(message)) => message.into(),
                Ok(None) => Event::Closed,
                Err(e) => Event::Failed(e.to_string()),
            };
            let last = matches!(event, Event::Closed | Event::Failed(_));
            if tx.unbounded_send(event).is_err() || last {
                break;
            }
        }
    });

    Ok((init, rx))
}

/// Writes one message to the core.
pub fn send(message: UIMessage) -> io::Result<()> {
    let mut bytes = CborVec::from_serializable(&message, 64).into_inner();

    let mut out = io::stdout().lock();
    let result = out.write_all(&bytes).and_then(|()| out.flush());

    bytes.zeroize();
    if let UIMessage::Password(mut secret) | UIMessage::PAMAnswer(mut secret) = message {
        secret.zeroize();
    }
    result
}

struct Incoming {
    stdin: io::Stdin,
    buf: Vec<u8>,
}

impl Incoming {
    /// Blocks until the initial state decodes. The core sends nothing else
    /// until we answer, so the buffer holds exactly this one item.
    /// (Decoded whole, not by prefix scan: it can carry large icons.)
    fn init(&mut self) -> io::Result<SelectUI> {
        loop {
            if let Ok(init) = cbor_deserialize(&self.buf) {
                self.buf.clear();
                return Ok(init);
            }
            if !self.read_more()? {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "core closed the pipe before sending state",
                ));
            }
        }
    }

    /// Blocks until one `ServiceMessage` is available. `None` on EOF.
    fn message(&mut self) -> io::Result<Option<ServiceMessage>> {
        loop {
            if let Some(message) = self.take_message() {
                return Ok(Some(message));
            }
            if self.buf.len() > MAX_BUFFERED {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "message from core too large",
                ));
            }
            if !self.read_more()? {
                return Ok(None);
            }
        }
    }

    /// The core can write several messages back to back (a PAM info line and
    /// its prompt, `AuthSelected` and then `FallbackToPassword`), and one read
    /// may return them all. CBOR is prefix-free, so the shortest prefix that
    /// decodes is exactly the first message. Messages are tiny, so the scan is cheap.
    fn take_message(&mut self) -> Option<ServiceMessage> {
        let len = (1..=self.buf.len())
            .find(|&len| cbor_deserialize::<ServiceMessage>(&self.buf[..len]).is_ok())?;
        let message = cbor_deserialize(&self.buf[..len]).ok()?;
        self.buf = self.buf.split_off(len);
        Some(message)
    }

    /// `false` on EOF. Blocking is fine: this runs on its own thread.
    fn read_more(&mut self) -> io::Result<bool> {
        let mut chunk = [0u8; 4096];
        let n = self.stdin.read(&mut chunk)?;
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(n != 0)
    }
}
