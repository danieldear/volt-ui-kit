//! Sign in with Apple in a Volt window.
//!
//! The signed-in user's identifier is kept in the Keychain. At launch the app
//! asks Apple whether that identifier still has access and signs out when the
//! person has revoked it in their Apple Account settings.
use volt_ui_kit::{
    native::{Application, UiSender},
    ui::prelude::*,
};
use volt_ui_kit_security::{
    apple_id::{self, AppleIdCredential, AppleIdError, AppleIdRequest, CredentialState, RealUser},
    secrets::{self, Secret, SecretError, SecretKey},
};

enum Message {
    SignedIn(Result<AppleIdCredential, AppleIdError>),
    Remembered(Result<Option<String>, SecretError>),
    Checked(Result<CredentialState, AppleIdError>),
    Saved(Result<(), SecretError>),
    Forgotten(Result<(), SecretError>),
}

/// What the person sees after signing in. Email and name come only the first
/// time; a real app saves them on its server then.
struct Account {
    user: String,
    email: Option<String>,
    name: Option<String>,
    token_bytes: Option<usize>,
    real_user: Option<RealUser>,
}

#[derive(Default)]
struct Demo {
    sender: Option<UiSender<Message>>,
    account: Option<Account>,
    access: Option<CredentialState>,
    status: String,
    working: bool,
}

fn user_key() -> SecretKey {
    SecretKey::new("dev.volt.signin-demo", "apple-user").expect("valid names")
}

impl Demo {
    fn send(&self) -> impl Fn(Message) + Send + 'static {
        let sender = self
            .sender
            .clone()
            .expect("installed before the first view");
        move |message| {
            let _ = sender.send(message);
        }
    }
    fn sign_in(&mut self) {
        let send = self.send();
        let request = AppleIdRequest::new().full_name().email();
        match apple_id::sign_in(&request, move |r| send(Message::SignedIn(r))) {
            Ok(()) => self.working = true,
            Err(e) => self.status = format!("{e}."),
        }
    }
    fn check(&mut self, user: &str) {
        let send = self.send();
        match apple_id::credential_state(user, move |r| send(Message::Checked(r))) {
            Ok(()) => self.working = true,
            Err(e) => self.status = format!("{e}."),
        }
    }
    fn sign_out(&mut self) {
        let send = self.send();
        let _ = secrets::remove(&user_key(), move |r| send(Message::Forgotten(r)));
        self.account = None;
        self.access = None;
        self.status = "Signed out on this Mac.".into();
    }
    fn update(&mut self, message: Message) {
        self.working = false;
        match message {
            Message::SignedIn(Ok(credential)) => {
                let send = self.send();
                let user = credential.user.clone();
                let _ = secrets::store(&user_key(), Secret::new(user), move |r| {
                    send(Message::Saved(r))
                });
                self.access = Some(CredentialState::Authorized);
                self.status =
                    "Signed in. A real app now sends the identity token to its server to verify."
                        .into();
                self.account = Some(Account {
                    user: credential.user,
                    email: credential.email,
                    name: credential.full_name,
                    token_bytes: credential.identity_token.as_ref().map(Secret::len),
                    real_user: Some(credential.real_user),
                });
            }
            Message::SignedIn(Err(AppleIdError::Canceled)) => self.status = "Canceled.".into(),
            Message::SignedIn(Err(e)) => self.status = format!("Couldn't sign in: {e}."),
            Message::Remembered(Ok(Some(user))) => {
                self.account = Some(Account {
                    user: user.clone(),
                    email: None,
                    name: None,
                    token_bytes: None,
                    real_user: None,
                });
                self.check(&user);
            }
            Message::Remembered(Ok(None)) => {}
            Message::Remembered(Err(e)) => self.status = format!("Keychain: {e}."),
            Message::Checked(Ok(state)) => {
                self.access = Some(state);
                if state != CredentialState::Authorized {
                    self.status = "Apple says this account no longer has access.".into();
                    self.sign_out();
                }
            }
            Message::Checked(Err(e)) => self.status = format!("Couldn't check access: {e}."),
            Message::Saved(result) | Message::Forgotten(result) => {
                if let Err(e) = result {
                    self.status = format!("Keychain: {e}.");
                }
            }
        }
    }
}

fn detail(label: &str, value: String) -> Element<Demo> {
    hstack![
        text(label).font_size(13.).color("#6B7280").width(110.),
        text(value).font_size(13.).flex(1.),
    ]
    .gap(8., Axis::Horizontal)
    .into()
}

fn view(app: &Demo) -> Element<Demo> {
    let mut rows: Vec<Element<Demo>> = vec![text("Sign in with Apple")
        .font_size(20.)
        .font_weight(600)
        .into()];
    if !apple_id::entitled() {
        rows.push(
            text("This copy isn't signed with the Sign in with Apple entitlement. Build it with cargo volt release and your provisioning profile (see README.md).")
                .key("not-entitled")
                .font_size(13.)
                .color("#B45309")
                .into(),
        );
    }
    match &app.account {
        None => rows.push(
            button("Sign in with Apple")
                .key("sign-in")
                .background("#000000")
                .color("#FFFFFF")
                .enabled(!app.working && apple_id::entitled())
                .on_click(Demo::sign_in)
                .into(),
        ),
        Some(account) => {
            let user = account.user.clone();
            let mut details = vec![detail("User", account.user.clone())];
            if let Some(name) = &account.name {
                details.push(detail("Name", name.clone()));
            }
            if let Some(email) = &account.email {
                details.push(detail("Email", email.clone()));
            }
            if let Some(bytes) = account.token_bytes {
                details.push(detail(
                    "Identity token",
                    format!("{bytes} bytes, for your server"),
                ));
            }
            if let Some(real) = account.real_user {
                details.push(detail("Real person", format!("{real:?}")));
            }
            if let Some(access) = app.access {
                details.push(detail("Access", format!("{access:?}")));
            }
            rows.push(
                vstack(details)
                    .key("account")
                    .gap(6., Axis::Vertical)
                    .into(),
            );
            rows.push(
                hstack![
                    button("Check access")
                        .key("check")
                        .variant(ButtonVariant::Soft)
                        .enabled(!app.working)
                        .on_click(move |app: &mut Demo| app.check(&user)),
                    button("Sign out")
                        .key("sign-out")
                        .variant(ButtonVariant::Soft)
                        .on_click(Demo::sign_out),
                ]
                .gap(8., Axis::Horizontal)
                .into(),
            );
        }
    }
    if !app.status.is_empty() {
        rows.push(
            text(&app.status)
                .key("status")
                .font_size(13.)
                .color("#6B7280")
                .into(),
        );
    }
    vstack(rows)
        .padding(24., Edge::All)
        .gap(14., Axis::Vertical)
        .align(Align::Stretch)
        .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Volt Sign in", Demo::default(), view)
        .size(520, 340)
        .messages(
            |app, sender| {
                app.sender = Some(sender.clone());
                // Someone signed in before: check they still have access.
                let _ = secrets::load(&user_key(), move |r| {
                    let user = r.map(|s| s.and_then(|s| s.expose_text().map(str::to_owned)));
                    let _ = sender.send(Message::Remembered(user));
                });
            },
            Demo::update,
        )
        .run()
}
