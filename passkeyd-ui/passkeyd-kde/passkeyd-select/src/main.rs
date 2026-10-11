mod ipc;
mod screen;
mod state;

use std::env;
use std::process::ExitCode;

use cxx_qt_lib::{QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use cxx_qt_lib_extras::QApplication;

fn main() -> ExitCode {
    passkeyd_locale::init_translations();

    // Block for the initial state before Qt starts; QML picks it up when it
    // creates `SelectionState`.
    match ipc::connect() {
        Ok((init, listener)) => state::hand_off(init, listener),
        Err(e) => {
            eprintln!("passkeyd-kde-select: {e}");
            return ExitCode::FAILURE;
        }
    }

    let mut app = QApplication::new();
    if env::var("QT_QUICK_CONTROLS_STYLE").is_err() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/passkeyd/ui/select/src/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    //! Plays the core: spawns the real binary under systemd-run, writes the
    //! initial state and `ServiceMessage`s to its stdin and reads the
    //! `UIMessage`s it sends back on stdout, exactly like `get.rs` does.
    //!
    //! Interactive: needs root, a desktop session for `GUI_UID` and you at the
    //! keyboard. There is no fingerprint reader involved, the sensor is
    //! simulated by waiting `STEP` and then sending the result.
    //!
    //!   cargo test -p passkeyd-kde-select -- --ignored --nocapture

    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
    use std::thread::sleep;
    use std::time::Duration;

    use ctap_types::Bytes;
    use ctap_types::serde::cbor_deserialize;
    use ctap_types::webauthn::{PublicKeyCredentialRpEntity, PublicKeyCredentialUserEntity};
    use passkeyd_abi::config::Auth;
    use passkeyd_abi::database::layout::OtherUI;
    use passkeyd_abi::utils::{
        CborVec, ConversationQuestion, FallbackToPasswordReason, SelectUI, ServiceMessage,
        UIMessage,
    };

    /// The desktop user the window is shown to.
    const GUI_UID: u32 = 1000;

    /// How long every non-interactive step stays on screen. This is also the
    /// simulated fingerprint sensor delay.
    const STEP: Duration = Duration::from_secs(1);

    const ACCOUNTS: usize = 6;

    #[test]
    #[ignore = "interactive: needs root, a desktop session and you clicking through the UI"]
    fn every_screen_and_message() {
        let mut child = spawn_ui();
        let mut core = Core::new(&mut child);
        core.send_state(&select_ui());

        core.scene("selection: pick an account");
        let index = core.hear_selection();
        println!("(picked account {index})");

        core.scene("waiting: choice sent, core is busy");
        core.pause();

        core.scene("fingerprint: sensor fails twice, the third read matches");
        core.say(ServiceMessage::AuthSelected(Auth::FPRINT));
        core.pause();
        core.say(ServiceMessage::Retry);
        core.pause();
        core.say(ServiceMessage::Retry);
        core.pause();
        println!("(sensor: match)");

        // These are password forms, so don't type while they are on screen.
        for reason in [
            FallbackToPasswordReason::FingerprintDeviceUnavailable,
            FallbackToPasswordReason::FingerprintUserUnavailable,
            FallbackToPasswordReason::NoEnrolledFingerprints,
            FallbackToPasswordReason::PamUnavailable,
        ] {
            core.scene(&format!("fallback to password: {reason:?}"));
            core.say(ServiceMessage::FallbackToPassword(reason));
            core.pause();
        }

        core.scene("password: type anything, the first answer is rejected, the second accepted");
        core.say(ServiceMessage::AuthSelected(Auth::PASS));
        core.hear_password();
        core.say(ServiceMessage::Retry);
        core.hear_password();
        println!("(password: accepted)");
        core.pause(); // the field stays locked while the core "decides"

        core.scene("pam: empty form, info line, hidden prompt, error + visible prompt");
        core.say(ServiceMessage::AuthSelected(Auth::PAM));
        core.pause();
        core.say(pam(ConversationQuestion::Info(
            "Authenticating as demo-user".into(),
        )));
        core.pause();
        core.say(pam(ConversationQuestion::SensitiveInput(
            "Password:".into(),
        )));
        core.hear_pam_answer();
        // Written back to back on purpose: the UI must split them.
        core.say(pam(ConversationQuestion::Error(
            "Permission denied, try again".into(),
        )));
        core.say(pam(ConversationQuestion::Input(
            "Verification code:".into(),
        )));
        core.hear_pam_answer();
        core.say(pam(ConversationQuestion::Info("Authenticated".into())));
        core.pause();

        core.scene("done: closing the pipe tells the UI to exit");
        drop(core);
        let status = child.wait().expect("Failed to collect UI exit status");
        assert!(status.success(), "UI exited with {status}");
    }

    fn pam(question: ConversationQuestion) -> ServiceMessage {
        ServiceMessage::PAMQuestion(question)
    }

    fn select_ui() -> SelectUI {
        SelectUI {
            rp: PublicKeyCredentialRpEntity {
                id: "github.com".into(),
                name: Some("Github".into()),
                icon: None,
            },
            other_uis: (0..ACCOUNTS)
                .map(|idx| OtherUI {
                    site_icon: None,
                    user_icon: None,
                    user: PublicKeyCredentialUserEntity {
                        id: Bytes::from_slice(&[idx as u8 + 1; 64])
                            .expect("Failed to create user ID"),
                        icon: None,
                        name: Some("Github".into()),
                        display_name: Some(format!("acc-{idx}").as_str().into()),
                    },
                })
                .collect(),
            no_pass: false,
        }
    }

    /// Builds the binary (so it is never stale) and returns its path.
    fn ui_binary() -> PathBuf {
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--manifest-path",
                concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"),
            ])
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "cargo build failed");

        // <target>/debug/deps/<this test binary>  ->  <target>/debug/passkeyd-kde-select
        let exe = std::env::current_exe().expect("Failed to locate the test binary");
        exe.parent()
            .and_then(|deps| deps.parent())
            .expect("Unexpected test binary location")
            .join("passkeyd-kde-select")
    }

    fn spawn_ui() -> Child {
        Command::new("systemd-run")
            .arg(format!("--machine={GUI_UID}@"))
            .arg("--user")
            .arg("--collect")
            .arg("--wait")
            .arg("--quiet")
            .arg("--pipe")
            .arg(ui_binary())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to spawn UI process. Are you root?")
    }

    /// The core's end of the pipes.
    struct Core {
        stdin: ChildStdin,
        stdout: ChildStdout,
        buf: Vec<u8>,
    }

    impl Core {
        fn new(child: &mut Child) -> Self {
            Self {
                stdin: child.stdin.take().expect("Failed to get stdin"),
                stdout: child.stdout.take().expect("Failed to get stdout"),
                buf: Vec::new(),
            }
        }

        fn scene(&self, title: &str) {
            println!("\n--- {title} ---");
        }

        /// Keeps the current screen up for one step.
        fn pause(&self) {
            sleep(STEP);
        }

        fn send_state(&mut self, state: &SelectUI) {
            println!(
                "[core -> ui] initial state ({} passkeys)",
                state.other_uis.len()
            );
            self.write(CborVec::from_serializable(state, 1024).into_inner());
        }

        fn say(&mut self, message: ServiceMessage) {
            println!("[core -> ui] {message:?}");
            self.write(CborVec::from_serializable(message, 64).into_inner());
        }

        fn write(&mut self, bytes: Vec<u8>) {
            self.stdin
                .write_all(&bytes)
                .and_then(|()| self.stdin.flush())
                .expect("Failed to write into pipe. Did the UI exit?");
        }

        /// Blocks until the UI sends a whole message, like `try_ui_message` does.
        fn hear(&mut self) -> UIMessage {
            let mut chunk = [0u8; 512];
            loop {
                if let Ok(message) = cbor_deserialize::<UIMessage>(&self.buf) {
                    self.buf.clear();
                    println!("[ui -> core] {}", describe(&message));
                    return message;
                }
                let n = self
                    .stdout
                    .read(&mut chunk)
                    .expect("Failed to read from the UI");
                assert!(
                    n != 0,
                    "UI closed its stdout before answering (window closed or crashed)"
                );
                self.buf.extend_from_slice(&chunk[..n]);
            }
        }

        fn hear_selection(&mut self) -> usize {
            match self.hear() {
                UIMessage::SelectionDoneMaybeStartAuth(index) => {
                    assert!(
                        index < ACCOUNTS,
                        "UI picked account {index}, only {ACCOUNTS} exist"
                    );
                    index
                }
                other => panic!("expected a selection, got {}", describe(&other)),
            }
        }

        fn hear_password(&mut self) {
            match self.hear() {
                UIMessage::Password(_) => {}
                other => panic!("expected Password, got {}", describe(&other)),
            }
        }

        fn hear_pam_answer(&mut self) {
            match self.hear() {
                UIMessage::PAMAnswer(_) => {}
                other => panic!("expected PAMAnswer, got {}", describe(&other)),
            }
        }
    }

    /// Secrets are never printed, only their length.
    fn describe(message: &UIMessage) -> String {
        match message {
            UIMessage::SelectionDoneMaybeStartAuth(index) => {
                format!("SelectionDoneMaybeStartAuth({index})")
            }
            UIMessage::Password(secret) => format!("Password(<{} chars>)", secret.chars().count()),
            UIMessage::PAMAnswer(answer) => {
                format!("PAMAnswer(<{} chars>)", answer.chars().count())
            }
        }
    }
}