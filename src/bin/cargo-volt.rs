//! Local scaffolding, rebuild/restart and macOS packaging. No shell evaluation.
#![forbid(unsafe_code)]
#[path = "volt/bundle.rs"]
mod bundle;
#[cfg(test)]
#[path = "volt/bundle_tests.rs"]
mod bundle_tests;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
fn template(kind: &str) -> io::Result<String> {
    if !["minimal", "navigation", "mail", "settings", "agent"].contains(&kind) {
        return Err(error(
            "template must be minimal, navigation, mail, settings or agent",
        ));
    }
    if kind == "minimal" {
        return Ok("use volt_ui_kit::{native::Application, ui::prelude::*};\nfn main() -> Result<(), Box<dyn std::error::Error>> {\n    Application::new(\"My app\", (), |_| vstack![text(\"Hello, Volt\").font_size(28.)].padding(24., Edge::All).into()).run()\n}\n".into());
    }
    let (title,items,body)=match kind {
        "mail"=>("Mailbox","Inbox|Sent|Drafts", "text(\"Your mail interface\").font_size(24.).into()"),
        "settings"=>("Settings","General|Appearance|Privacy", "toggle(\"Enable feature\", app.enabled).key(\"enabled\").on_change(|app: &mut App, value| app.enabled = value).into()"),
        "agent"=>("Agent workspace","Chats|Tasks|History", "text_area(&app.draft).key(\"draft\").height(160.).on_change(|app: &mut App, value| app.draft = value).into()"),
        _=>("Workspace","Overview|Projects|Activity", "text(format!(\"Selected destination: {}\", app.page)).font_size(24.).into()"),
    };
    let extra_state = match kind {
        "settings" => ", enabled: bool",
        "agent" => ", draft: String",
        _ => "",
    };
    let items = items
        .split('|')
        .enumerate()
        .map(|(i, label)| format!("navigation_item({i}u8, \"{label}\").key(\"destination-{i}\")"))
        .collect::<Vec<_>>()
        .join(",\n            ");
    Ok(format!(
        r#"use volt_ui_kit::{{native::Application, ui::prelude::*}};
#[derive(Default)]
struct App {{ page: u8{extra_state} }}
fn view(app: &App) -> Element<App> {{
    let content: Element<App> = {body};
    hstack![
        navigation([
            {items}
        ]).key("navigation").selected(app.page).width(200.)
            .on_select(|app: &mut App, page| app.page = page),
        vstack![text("{title}").font_size(30.).font_weight(700), content]
            .flex(1.).padding(24., Edge::All).gap(16., Axis::Vertical),
    ].gap(16., Axis::Horizontal).padding(16., Edge::All).into()
}}
fn main() -> Result<(), Box<dyn std::error::Error>> {{
    // No prescribed shell, opacity or provider. Choose appearance in Rust.
    Application::new("{title}", App::default(), view).size(1100, 760).run()
}}
"#
    ))
}
fn desktop_template(source: String, kind: &str) -> String {
    let callback = if kind == "minimal" {
        "|_: &mut (), id| println!(\"Native command: {id}\")"
    } else if kind == "agent" {
        "|app: &mut App, id| { if id == \"settings\" { app.page = 2; } else if id == \"clear\" { app.draft.clear(); } }"
    } else {
        "|app: &mut App, id| { if id == \"settings\" { app.page = 2; } }"
    };
    let source = source.replace(".run()", &format!(".desktop(desktop(), {callback}).run()"));
    let desktop = source
        + r#"
fn desktop() -> volt_ui_kit::native::system::DesktopConfig {
    use volt_ui_kit::native::system::{DesktopConfig, DesktopMenu, DesktopCommand, TrayConfig};
    let commands = vec![DesktopCommand::new("settings", "Settings"), DesktopCommand::new("clear", "Clear draft")];
    let rgba = (0..16 * 16).flat_map(|n| {
        let (x,y) = (n % 16 - 8i32, n / 16 - 8i32);
        [255,255,255,if x*x+y*y <= 36 {255} else {0}]
    }).collect();
    DesktopConfig::new("Volt App").menu(DesktopMenu::new("Workspace", commands.clone()))
        .tray(TrayConfig { tooltip: "Volt App".into(), rgba, width: 16, height: 16,
            template: true, commands })
}
"#;
    if kind == "agent" || kind == "minimal" {
        desktop
    } else {
        desktop.replace(", DesktopCommand::new(\"clear\", \"Clear draft\")", "")
    }
}
fn scaffold(destination: &Path, kind: &str, kit: &Path, desktop: bool) -> io::Result<()> {
    let source = template(kind)?;
    let source = if desktop {
        desktop_template(source, kind)
    } else {
        source
    };
    let features = if desktop {
        "\"native-gpu\", \"native-desktop\""
    } else {
        "\"native-gpu\""
    };
    let name = destination
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| error("invalid project name"))?;
    if name.is_empty()
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(error(
            "project name must start with a letter and use ASCII letters/digits/-/_",
        ));
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(error("destination already exists; nothing overwritten"));
    }
    let kit = kit.canonicalize()?;
    // A local source dependency is required until the toolkit is published.
    let manifest = fs::read_to_string(kit.join("Cargo.toml"))?;
    if !manifest.contains("name = \"volt-ui-kit\"") {
        return Err(error("--kit-path is not Volt UI Kit"));
    }
    let kit = kit
        .to_str()
        .ok_or_else(|| error("toolkit path must be UTF-8"))?;
    let path = serde_json::to_string(kit).map_err(|e| error(e.to_string()))?;
    // create_dir is atomic and refuses an existing/symlink destination.
    fs::create_dir(destination)?;
    let result = (|| {
        fs::create_dir(destination.join("src"))?;
        fs::write(destination.join("Cargo.toml"),format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nresolver = \"3\"\nrust-version = \"1.87\"\n\n[dependencies]\nvolt-ui-kit = {{ path = {path}, features = [{features}] }}\n"))?;
        fs::write(destination.join("src/main.rs"), source)?;
        fs::write(destination.join(".gitignore"), "/target\n")?;
        fs::write(destination.join("README.md"),format!("# {name}\n\nRun `cargo run`. GPU is preferred; no renderer flags needed.\nThis {kind} template is a UI starting point, not a mail backend, AI client or editor engine.\nCustomize layouts, themes and window backgrounds in Rust.\n\nUse `cargo-volt dev .` for rebuild/restart (state is not retained).\n\nOn macOS, build with `cargo build --release`, then see `cargo-volt bundle --help`\nfor a local .app with explicit identity/resources and ad-hoc signing. This is not\nDeveloper ID signing or notarization; keep bundle resources immutable.\n"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}
fn fingerprint(path: &Path, hash: &mut u64) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(());
    }
    if path.is_dir() {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            if name == "target" || name == ".git" || name == "Cargo.lock" {
                continue;
            }
            fingerprint(&entry.path(), hash)?;
        }
    } else {
        // Watch metadata, not unbounded content; this is rebuild/restart, not hot reload.
        let metadata = fs::metadata(path)?;
        for b in format!(
            "{}:{}:{:?}",
            path.display(),
            metadata.len(),
            metadata.modified()?
        )
        .bytes()
        {
            *hash = (*hash ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
    }
    Ok(())
}
struct Running(Option<Child>);
impl Running {
    fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop();
    }
}
fn rebuild(root: &Path) -> io::Result<Option<PathBuf>> {
    let output = Command::new("cargo")
        .args(["build", "--message-format=json-render-diagnostics"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()?;
    let mut binary = None;
    for line in output.stdout.split(|b| *b == b'\n') {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if value["reason"] == "compiler-message" {
            if let Some(rendered) = value["message"]["rendered"].as_str() {
                eprint!("{rendered}");
            }
        }
        if value["reason"] == "compiler-artifact"
            && value["target"]["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|v| v == "bin"))
            && value["profile"]["test"] == false
        {
            if let Some(executable) = value["executable"].as_str() {
                if binary.is_some() {
                    return Err(error("dev requires a project with one binary target"));
                }
                binary = Some(PathBuf::from(executable));
            }
        }
    }
    Ok(if output.status.success() {
        binary
    } else {
        None
    })
}
fn dev(root: &Path) -> io::Result<()> {
    let root = root.canonicalize()?;
    let mut running = Running(None);
    let mut last = None;
    loop {
        let mut hash = 0xcbf29ce484222325;
        // Only source/manifests/config, never generated logs/assets/target.
        for path in [
            root.join("src"),
            root.join("Cargo.toml"),
            root.join(".cargo"),
        ] {
            if path.exists() {
                fingerprint(&path, &mut hash)?;
            }
        }
        if last != Some(hash) {
            last = Some(hash);
            if let Some(binary) = rebuild(&root)? {
                running.stop();
                running.0 = Some(Command::new(binary).current_dir(&root).spawn()?);
            } else {
                eprintln!(
                    "Build failed/no executable; previous app retained. Save a change to retry."
                );
            }
        }
        if let Some(child) = running.0.as_mut() {
            if child.try_wait()?.is_some() {
                running.0 = None;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "volt") {
        args.remove(0);
    }
    match args.first().map(String::as_str) {
        Some("new") if args.len()>=2 => {
            let mut kind="minimal"; let mut kit=None; let mut desktop=false; let mut i=2;
            while i<args.len() {match args[i].as_str() {
                "--desktop" => {desktop=true;i+=1;},
                "--template" if i+1<args.len()=>{kind=&args[i+1];i+=2;},
                "--kit-path" if i+1<args.len()=>{kit=Some(Path::new(&args[i+1]));i+=2;},
                _=>return Err(error("unknown/missing scaffold option").into()),
            }}
            scaffold(Path::new(&args[1]),kind,kit.ok_or_else(||error("--kit-path required until published"))?, desktop)?;
            println!("Created {}. Run: cargo run --manifest-path {}/Cargo.toml",args[1],args[1]);
        }
        Some("bundle") if args.get(1).is_some_and(|s| s == "--help") && args.len()==2 => println!("{}", bundle::HELP),
        Some("bundle") => bundle::run(bundle::BundleOptions::parse(&args[1..])?)?,
        Some("dev") if args.len()<=2=>dev(Path::new(args.get(1).map_or(".",String::as_str)))?,
        _=>return Err(error("Usage: cargo volt new PATH --kit-path PATH [--template minimal|navigation|mail|settings|agent] [--desktop]\n       cargo volt dev [PATH] (runs trusted local project; rebuild/restart, not state-preserving hot reload)\n       cargo volt bundle --help").into()),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_template_uses_application_and_does_not_claim_a_backend() {
        for kind in ["minimal", "navigation", "mail", "settings", "agent"] {
            let s = template(kind).unwrap();
            assert!(s.contains("Application::new"));
        }
        assert!(template("unknown").is_err());
    }
    #[test]
    fn scaffold_never_overwrites_and_preserves_custom_paths() {
        let dir = std::env::temp_dir().join(format!("volt-scaffold-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let dest = dir.join("mail-app");
        scaffold(&dest, "mail", Path::new(env!("CARGO_MANIFEST_DIR")), false).unwrap();
        let original = fs::read(dest.join("Cargo.toml")).unwrap();
        assert!(String::from_utf8_lossy(&original).contains("resolver = \"3\""));
        assert!(scaffold(
            &dest,
            "settings",
            Path::new(env!("CARGO_MANIFEST_DIR")),
            false
        )
        .is_err());
        assert_eq!(fs::read(dest.join("Cargo.toml")).unwrap(), original);
        assert!(scaffold(
            &dir.join("bad;name"),
            "minimal",
            Path::new(env!("CARGO_MANIFEST_DIR")),
            false
        )
        .is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
