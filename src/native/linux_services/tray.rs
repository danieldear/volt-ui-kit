//! StatusNotifierItem + DBusMenu, without GTK. Registration is desktop-dependent.
use crate::native::{
    system::{DesktopCommand, DesktopConfig},
    CommandInfo,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Str, Value};
type Properties = HashMap<String, OwnedValue>;
type Layout = (i32, Properties, Vec<OwnedValue>);
type Pixmaps = Vec<(i32, i32, Vec<u8>)>;
#[derive(Clone)]
struct Entry {
    command: DesktopCommand,
    enabled: bool,
    quit: bool,
}
struct State {
    title: String,
    id: String,
    pixmaps: Pixmaps,
    entries: Vec<Entry>,
    revision: u32,
    send: Arc<dyn Fn(DesktopEvent) + Send + Sync>,
}
#[derive(Clone)]
struct Item(Arc<Mutex<State>>);
#[derive(Clone)]
struct Menu(Arc<Mutex<State>>);
pub(in crate::native) enum DesktopEvent {
    Command(String),
    Quit,
}
fn properties(entry: &Entry) -> Properties {
    let mut p = Properties::new();
    if entry.command.separator {
        p.insert("type".into(), Str::from("separator").into());
    } else {
        p.insert(
            "label".into(),
            Str::from(entry.command.label.as_str()).into(),
        );
        p.insert("enabled".into(), entry.enabled.into());
    }
    p.insert("visible".into(), true.into());
    p
}
fn activate(state: &Arc<Mutex<State>>, id: i32) {
    let event = {
        let s = state.lock().unwrap_or_else(|p| p.into_inner());
        s.entries
            .get(id.saturating_sub(1) as usize)
            .filter(|_| id > 0)
            .filter(|e| e.enabled && !e.command.separator)
            .map(|e| (s.send.clone(), e.command.id.clone(), e.quit))
    };
    if let Some((send, id, quit)) = event {
        if quit {
            send(DesktopEvent::Quit)
        } else {
            send(DesktopEvent::Command(id))
        }
    }
}
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn category(&self) -> &str {
        "ApplicationStatus"
    }
    #[zbus(property)]
    fn id(&self) -> String {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).id.clone()
    }
    #[zbus(property)]
    fn title(&self) -> String {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .title
            .clone()
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }
    #[zbus(property)]
    fn window_id(&self) -> i32 {
        0
    }
    #[zbus(property)]
    fn icon_theme_path(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn icon_pixmap(&self) -> Pixmaps {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pixmaps
            .clone()
    }
    #[zbus(property)]
    fn attention_icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn attention_icon_pixmap(&self) -> Pixmaps {
        Vec::new()
    }
    #[zbus(property)]
    fn overlay_icon_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn overlay_icon_pixmap(&self) -> Pixmaps {
        Vec::new()
    }
    #[zbus(property)]
    fn attention_movie_name(&self) -> &str {
        ""
    }
    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn menu(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from("/MenuBar").expect("static object path")
    }
    #[zbus(property, name = "ToolTip")]
    fn tool_tip(&self) -> (String, Pixmaps, String, String) {
        (
            String::new(),
            self.icon_pixmap(),
            self.title(),
            String::new(),
        )
    }
    fn activate(&self, _x: i32, _y: i32) {
        activate(&self.0, 1);
    }
    fn secondary_activate(&self, _x: i32, _y: i32) {}
    fn context_menu(&self, _x: i32, _y: i32) {}
    fn scroll(&self, _delta: i32, _orientation: &str) {}
}
#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }
    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }
    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }
    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, Layout)> {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if parent_id == 0 {
            let mut root = Properties::new();
            root.insert("children-display".into(), Str::from("submenu").into());
            let children = if recursion_depth == 0 {
                Vec::new()
            } else {
                state
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        OwnedValue::try_from(Value::new((
                            i as i32 + 1,
                            properties(e),
                            Vec::<OwnedValue>::new(),
                        )))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?
            };
            Ok((state.revision, (0, root, children)))
        } else {
            let entry = state
                .entries
                .get(
                    parent_id
                        .checked_sub(1)
                        .ok_or_else(|| zbus::fdo::Error::InvalidArgs("unknown item".into()))?
                        as usize,
                )
                .ok_or_else(|| zbus::fdo::Error::InvalidArgs("unknown item".into()))?;
            Ok((state.revision, (parent_id, properties(entry), Vec::new())))
        }
    }
    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, Properties)> {
        let s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.entries
            .iter()
            .enumerate()
            .filter(|(i, _)| ids.is_empty() || ids.contains(&(*i as i32 + 1)))
            .map(|(i, e)| (i as i32 + 1, properties(e)))
            .collect()
    }
    fn get_property(&self, id: i32, name: &str) -> zbus::fdo::Result<OwnedValue> {
        let s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.entries
            .get(id.saturating_sub(1) as usize)
            .filter(|_| id > 0)
            .and_then(|e| properties(e).remove(name))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs("unknown item/property".into()))
    }
    fn event(&self, id: i32, event_id: &str, _data: OwnedValue, _timestamp: u32) {
        if event_id == "clicked" {
            activate(&self.0, id);
        }
    }
    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        let mut errors = Vec::new();
        for (id, event, data, time) in events.into_iter().take(512) {
            if id <= 0
                || id as usize
                    > self
                        .0
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .entries
                        .len()
            {
                errors.push(id);
            } else {
                self.event(id, &event, data, time);
            }
        }
        errors
    }
    fn about_to_show(&self, _id: i32) -> bool {
        false
    }
    fn about_to_show_group(&self, ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        let count = self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entries
            .len() as i32;
        (
            Vec::new(),
            ids.into_iter()
                .filter(|id| *id < 0 || *id > count)
                .collect(),
        )
    }
}
pub(in crate::native) struct DesktopHost {
    state: Arc<Mutex<State>>,
    wake: async_channel::Sender<()>,
}
impl DesktopHost {
    pub fn install(
        config: &DesktopConfig,
        send: impl Fn(DesktopEvent) + Send + Sync + 'static,
        guarded_quit: bool,
        commands: &[CommandInfo<'_>],
    ) -> Result<Self, String> {
        config.validate()?;
        let mut entries = config
            .tray
            .as_ref()
            .map(|t| t.commands.clone())
            .unwrap_or_default();
        if guarded_quit && config.tray.is_some() {
            entries.push(DesktopCommand::separator());
            entries.push(DesktopCommand::new("volt-host-quit", "Quit"));
        }
        let pixmaps = config
            .tray
            .as_ref()
            .map(|t| {
                let argb = t
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|c| [c[3], c[0], c[1], c[2]])
                    .collect();
                vec![(t.width as i32, t.height as i32, argb)]
            })
            .unwrap_or_default();
        let state = Arc::new(Mutex::new(State {
            title: config
                .tray
                .as_ref()
                .map_or_else(|| config.app_name.clone(), |t| t.tooltip.clone()),
            id: super::app_id().into(),
            pixmaps,
            entries: entries
                .into_iter()
                .enumerate()
                .map(|(index, command)| Entry {
                    enabled: command.enabled,
                    quit: guarded_quit
                        && config.tray.is_some()
                        && index + 1 == config.tray.as_ref().map_or(0, |t| t.commands.len()) + 2,
                    command,
                })
                .collect(),
            revision: 1,
            send: Arc::new(send),
        }));
        let (tx, rx) = async_channel::bounded(1);
        let host = Self {
            state: state.clone(),
            wake: tx,
        };
        host.sync_commands(commands, true);
        if config.tray.is_some() {
            std::thread::Builder::new()
                .name("volt-tray".into())
                .spawn(move || {
                    if let Err(error) = serve(state, rx) {
                        eprintln!("Volt Linux tray unavailable: {error}");
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(host)
    }
    pub fn sync_commands(&self, commands: &[CommandInfo<'_>], allowed: bool) {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        let mut changed = false;
        for entry in &mut s.entries {
            let enabled = allowed
                && entry.command.enabled
                && commands
                    .iter()
                    .find(|c| c.id == entry.command.id)
                    .is_none_or(|c| c.enabled);
            changed |= enabled != entry.enabled;
            entry.enabled = enabled;
        }
        if changed {
            s.revision = s.revision.wrapping_add(1);
        }
        drop(s);
        if changed {
            let _ = self.wake.try_send(());
        }
    }
}
fn serve(state: Arc<Mutex<State>>, rx: async_channel::Receiver<()>) -> Result<(), String> {
    use futures_lite::StreamExt;
    let name = format!(
        "org.kde.StatusNotifierItem-{}-{}",
        std::process::id(),
        super::TOKENS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let conn = zbus::blocking::connection::Builder::session()
        .map_err(|e| e.to_string())?
        .method_timeout(Duration::from_secs(5))
        .name(name.as_str())
        .map_err(|e| e.to_string())?
        .serve_at("/StatusNotifierItem", Item(state.clone()))
        .map_err(|e| e.to_string())?
        .serve_at("/MenuBar", Menu(state.clone()))
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    futures_lite::future::block_on(async {
        let bus = zbus::Proxy::new(
            conn.inner(),
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )
        .await
        .map_err(|e| e.to_string())?;
        let mut owners = bus
            .receive_signal_with_args("NameOwnerChanged", &[(0, "org.kde.StatusNotifierWatcher")])
            .await
            .map_err(|e| e.to_string())?;
        let watcher = zbus::Proxy::new(
            conn.inner(),
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.kde.StatusNotifierWatcher",
        )
        .await
        .map_err(|e| e.to_string())?;
        if let Err(e) = watcher
            .call::<_, _, ()>("RegisterStatusNotifierItem", &(name.as_str(),))
            .await
        {
            eprintln!("Volt Linux tray: no StatusNotifierWatcher; waiting for one ({e})");
        }
        loop {
            let action = futures_lite::future::or(
                async {
                    match rx.recv().await {
                        Ok(()) => 1,
                        Err(_) => 0,
                    }
                },
                async {
                    if owners.next().await.is_some() {
                        2
                    } else {
                        0
                    }
                },
            )
            .await;
            match action {
                0 => return Ok(()),
                2 => {
                    let _ = watcher
                        .call::<_, _, ()>("RegisterStatusNotifierItem", &(name.as_str(),))
                        .await;
                }
                _ => {
                    let revision = state.lock().unwrap_or_else(|p| p.into_inner()).revision;
                    let _ = conn
                        .inner()
                        .emit_signal(
                            None::<&str>,
                            "/MenuBar",
                            "com.canonical.dbusmenu",
                            "LayoutUpdated",
                            &(revision, 0i32),
                        )
                        .await;
                }
            }
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_menu_items_never_dispatch() {
        let hits = Arc::new(Mutex::new(Vec::new()));
        let output = hits.clone();
        let state = Arc::new(Mutex::new(State {
            title: "QA".into(),
            id: "dev.volt.qa".into(),
            pixmaps: vec![],
            entries: vec![Entry {
                command: DesktopCommand::new("one", "One"),
                enabled: false,
                quit: false,
            }],
            revision: 1,
            send: Arc::new(move |e| {
                if let DesktopEvent::Command(id) = e {
                    output.lock().unwrap().push(id);
                }
            }),
        }));
        let item = Item(state.clone());
        let _: i32 = item.window_id();
        let menu = Menu(state.clone());
        let (_, layout) = menu.get_layout(0, -1, Vec::new()).unwrap();
        assert_eq!(layout.2.len(), 1);
        assert_eq!(
            <Layout as zbus::zvariant::Type>::SIGNATURE.to_string(),
            "(ia{sv}av)"
        );
        activate(&state, 1);
        activate(&state, -1);
        assert!(hits.lock().unwrap().is_empty());
        state.lock().unwrap().entries[0].enabled = true;
        activate(&state, 1);
        assert_eq!(*hits.lock().unwrap(), ["one"]);
    }
}
