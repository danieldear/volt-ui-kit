//! A key kept in the login Keychain and shown only after Touch ID or the
//! account password.
//!
//! cargo run --example vault   (from crates/volt-ui-kit-security)
//!
//! Save puts the key in the Keychain; nothing is written to disk by the app.
//! Show asks macOS to confirm it's you, then reads the key back. Remove
//! deletes it. Keychain Access lists the item as "dev.volt.vault-demo".
use volt_ui_kit::{
    native::{Application, UiSender},
    ui::prelude::*,
};
use volt_ui_kit_security::{
    local_auth::{self, Biometry, LocalAuthError, Policy},
    secrets::{self, Secret, SecretError, SecretKey},
};

enum Message {
    Found(Result<bool, SecretError>),
    Saved(Result<(), SecretError>),
    Confirmed(Result<(), LocalAuthError>),
    Read(Result<Option<String>, SecretError>),
    Removed(Result<(), SecretError>),
}

#[derive(Default)]
struct Vault {
    sender: Option<UiSender<Message>>,
    draft: String,
    /// Whether the Keychain holds a key: unknown until the first read.
    stored: Option<bool>,
    /// The key, only between a confirmation and Hide.
    shown: Option<String>,
    status: String,
    working: bool,
}

fn key() -> SecretKey {
    SecretKey::new("dev.volt.vault-demo", "api-key").expect("valid names")
}

impl Vault {
    fn send(&self) -> impl Fn(Message) + Send + 'static {
        let sender = self
            .sender
            .clone()
            .expect("installed before the first view");
        move |message| {
            let _ = sender.send(message);
        }
    }
    /// Starts an operation, or shows why it couldn't start.
    fn start(&mut self, started: Result<(), String>) {
        match started {
            Ok(()) => self.working = true,
            Err(error) => self.status = error,
        }
    }
    fn save(&mut self) {
        let draft = std::mem::take(&mut self.draft);
        let send = self.send();
        let started = secrets::store(&key(), Secret::new(draft), move |r| send(Message::Saved(r)));
        self.start(started.map_err(|e| e.to_string()));
    }
    fn show(&mut self) {
        let send = self.send();
        let started = local_auth::confirm(
            "show the saved key",
            Policy::BiometricsOrPassword,
            move |r| send(Message::Confirmed(r)),
        );
        self.start(started.map_err(|e| e.to_string()));
    }
    fn remove(&mut self) {
        let send = self.send();
        let started = secrets::remove(&key(), move |r| send(Message::Removed(r)));
        self.start(started.map_err(|e| e.to_string()));
    }
    fn update(&mut self, message: Message) {
        self.working = false;
        match message {
            Message::Found(Ok(found)) => self.stored = Some(found),
            Message::Saved(Ok(())) => {
                self.stored = Some(true);
                self.shown = None;
                self.status = "Saved to the Keychain.".into();
            }
            Message::Confirmed(Ok(())) => {
                let send = self.send();
                let started = secrets::load(&key(), move |r| {
                    let text = r.map(|s| s.and_then(|s| s.expose_text().map(str::to_owned)));
                    send(Message::Read(text))
                });
                self.start(started.map_err(|e| e.to_string()));
            }
            Message::Read(Ok(Some(text))) => {
                self.shown = Some(text);
                self.status.clear();
            }
            Message::Read(Ok(None)) => {
                self.stored = Some(false);
                self.status = "The Keychain has no key for this demo.".into();
            }
            Message::Removed(Ok(())) => {
                self.stored = Some(false);
                self.shown = None;
                self.status = "Removed from the Keychain.".into();
            }
            Message::Confirmed(Err(LocalAuthError::Canceled)) => self.status = "Canceled.".into(),
            Message::Confirmed(Err(e)) => self.status = format!("Couldn't confirm: {e}."),
            Message::Found(Err(e))
            | Message::Saved(Err(e))
            | Message::Read(Err(e))
            | Message::Removed(Err(e)) => self.status = format!("Keychain: {e}."),
        }
    }
}

fn how_to_confirm() -> String {
    match local_auth::check(Policy::BiometricsOrPassword) {
        Ok(Biometry::TouchId) => "Show asks for Touch ID, or your password.".into(),
        Ok(_) => "Show asks for your account password.".into(),
        Err(e) => format!("Show can't confirm on this Mac: {e}."),
    }
}

fn view(app: &Vault) -> Element<Vault> {
    let stored = app.stored == Some(true);
    let mut rows: Vec<Element<Vault>> = vec![
        text("Keychain demo").font_size(20.).font_weight(600).into(),
        text(how_to_confirm())
            .font_size(13.)
            .color("#6B7280")
            .into(),
        hstack![
            text_field(&app.draft)
                .key("draft")
                .password(true)
                .placeholder(if stored {
                    "Paste a new key"
                } else {
                    "Paste a key"
                })
                .accessible_label("Key")
                .on_change(|app: &mut Vault, value| app.draft = value)
                .on_submit(|app: &mut Vault| {
                    if !app.draft.trim().is_empty() {
                        app.save()
                    }
                })
                .flex(1.),
            button("Save")
                .key("save")
                .enabled(!app.draft.trim().is_empty() && !app.working)
                .on_click(Vault::save),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    ];
    let shown: Element<Vault> = match (&app.shown, app.stored) {
        (Some(key), _) => text(key).key("shown").font_family("monospace").into(),
        (None, Some(true)) => text("A key is saved. It stays hidden until you confirm.")
            .key("shown")
            .into(),
        (None, Some(false)) => text("No key saved yet.").key("shown").into(),
        (None, None) => text("Checking the Keychain…").key("shown").into(),
    };
    rows.push(shown);
    rows.push(
        hstack![
            if app.shown.is_some() {
                button("Hide")
                    .key("show")
                    .on_click(|app: &mut Vault| app.shown = None)
            } else {
                button("Show")
                    .key("show")
                    .variant(ButtonVariant::Soft)
                    .enabled(stored && !app.working)
                    .on_click(Vault::show)
            },
            button("Remove")
                .key("remove")
                .variant(ButtonVariant::Soft)
                .enabled(stored && !app.working)
                .on_click(Vault::remove),
        ]
        .gap(8., Axis::Horizontal)
        .into(),
    );
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
    Application::new("Keychain demo", Vault::default(), view)
        .size(460, 300)
        .messages(
            |app, sender| {
                app.sender = Some(sender.clone());
                // Only whether a key exists: the secret itself is dropped here.
                let found = secrets::load(&key(), move |r| {
                    let _ = sender.send(Message::Found(r.map(|s| s.is_some())));
                });
                if let Err(e) = found {
                    app.status = e.to_string();
                }
            },
            Vault::update,
        )
        .run()
}
