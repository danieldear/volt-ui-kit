//! Explicit per-user Linux registration. No build, shell, sudo or compositor changes.
use std::{
    collections::HashSet,
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const DEFAULT_ICON: &str = include_str!("app-icon.svg");
pub const HELP: &str = "cargo volt install-linux --binary PATH --app-id com.example.app --name 'My App' --icon PATH.svg|PATH.png [--replace] [--data-dir PATH]\n\nLinux only. Registers an already-built executable in the current user's application menu.\nThe executable stays at its canonical path: do not move/delete it after registration.\nAccepts an SVG or 256x256 PNG; copies it to XDG_DATA_HOME/icons/hicolor and writes a matching .desktop entry\nin XDG_DATA_HOME/applications (default ~/.local/share). No sudo, launch or permissions prompt.\n--replace updates only entries previously registered by this command, with the same app ID.\n--data-dir must be absolute; use it for isolated packaging tests. Ordinary cargo run/new never registers apps.\nUse the same ID in Application::app_id(). Restart running apps after registering.\nExecutable paths containing % or = are refused for desktop/GIO compatibility.\nChanging from SVG to PNG (or vice versa) on update is not supported; keep the format stable.";
const LIMIT: u64 = 4 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
#[derive(Debug)]
pub struct InstallOptions {
    pub binary: PathBuf,
    pub app_id: String,
    pub name: String,
    pub icon: PathBuf,
    pub data_dir: PathBuf,
    pub replace: bool,
}
impl InstallOptions {
    pub fn parse(args: &[String]) -> io::Result<Self> {
        let mut seen = HashSet::new();
        let mut values = std::collections::HashMap::new();
        let mut replace = false;
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            if !seen.insert(flag) {
                return Err(error(format!("duplicate option {flag}")));
            }
            if flag == "--replace" {
                replace = true;
                i += 1;
                continue;
            }
            if !["--binary", "--app-id", "--name", "--icon", "--data-dir"].contains(&flag) {
                return Err(error(format!("unknown option {flag}")));
            }
            let value = args
                .get(i + 1)
                .filter(|v| !v.starts_with("--"))
                .ok_or_else(|| error(format!("missing value for {flag}")))?;
            values.insert(flag, value.clone());
            i += 2;
        }
        let mut required = |flag| {
            values
                .remove(flag)
                .ok_or_else(|| error(format!("{flag} is required")))
        };
        let binary = required("--binary")?.into();
        let app_id = required("--app-id")?;
        let name = required("--name")?;
        let icon = required("--icon")?.into();
        let data_dir = values
            .remove("--data-dir")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("XDG_DATA_HOME")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
            })
            .ok_or_else(|| error("set HOME, XDG_DATA_HOME or --data-dir"))?;
        Ok(Self {
            binary,
            app_id,
            name,
            icon,
            data_dir,
            replace,
        })
    }
}
#[derive(Debug)]
pub struct Installation {
    pub launcher: PathBuf,
    pub icon: PathBuf,
}
fn text(value: &str) -> io::Result<&str> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(error(
            "names and paths must be nonempty and contain no control characters",
        ))
    } else {
        Ok(value)
    }
}
fn path_text(path: &Path) -> io::Result<&str> {
    text(
        path.to_str()
            .ok_or_else(|| error("desktop paths must be UTF-8"))?,
    )
}
fn id_valid(id: &str) -> bool {
    id.len() <= 247
        && id.contains('.')
        && id.split('.').all(|part| {
            !part.is_empty()
                && (part.as_bytes()[0].is_ascii_alphabetic() || part.starts_with('_'))
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}
// Desktop Entry string escaping is applied after Exec's own argument quoting.
fn value_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace(' ', "\\s")
}
fn exec_argument(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' | '"' | '$' | '`' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    value_escape(&quoted)
}
fn existing(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= LIMIT => Ok(Some(fs::read(path)?)),
        Ok(_) => Err(error(format!(
            "refusing symlink, directory or oversized destination: {}",
            path.display()
        ))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn publish(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| error("missing destination parent"))?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".volt-install-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if replace {
            fs::rename(&temp, path)
        } else {
            fs::hard_link(&temp, path)
        }
    })();
    let _ = fs::remove_file(&temp);
    result
}
pub fn install(options: &InstallOptions) -> io::Result<Installation> {
    if !id_valid(&options.app_id) {
        return Err(error(
            "app ID must be reverse-DNS ASCII with letters/digits/underscores, at most 247 bytes",
        ));
    }
    text(&options.name)?;
    path_text(&options.data_dir)?;
    if !options.data_dir.is_absolute() {
        return Err(error("XDG_DATA_HOME / --data-dir must be absolute"));
    }
    let binary = fs::canonicalize(&options.binary)?;
    let binary_text = path_text(&binary)?;
    // Desktop Entry Exec forbids '=' in executable names. GLib's initial
    // executable check does not unescape %% even though launch expansion does;
    // reject '%' rather than installing an entry that GIO silently hides.
    if binary_text.contains(['%', '=']) {
        return Err(error("executable path cannot contain % or = for portable desktop launchers; build/move it to another path before registering"));
    }
    let metadata = fs::metadata(&binary)?;
    if !metadata.is_file() {
        return Err(error("--binary must be a regular executable file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(error("--binary is not executable"));
        }
    }
    let extension = options
        .icon
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("--icon requires .svg or .png"))?;
    if !["svg", "png"].contains(&extension) {
        return Err(error("--icon requires .svg or .png"));
    }
    if !fs::metadata(&options.icon)?.is_file() {
        return Err(error("--icon must be a file"));
    }
    let mut image = Vec::new();
    fs::File::open(&options.icon)?
        .take(LIMIT + 1)
        .read_to_end(&mut image)?;
    if image.len() as u64 > LIMIT
        || !(if extension == "png" {
            image.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
                && image.get(16..24) == Some(&[0, 0, 1, 0, 0, 0, 1, 0])
        } else {
            std::str::from_utf8(&image)
                .is_ok_and(|s| s.contains("<svg") && s.contains("http://www.w3.org/2000/svg"))
        })
    {
        return Err(error(
            "icon must have an SVG or 256x256 PNG header and at most 4 MiB (artwork is not decoded)",
        ));
    }
    let id = &options.app_id;
    let result = Installation {
        launcher: options.data_dir.join(format!("applications/{id}.desktop")),
        icon: options.data_dir.join(format!(
            "icons/hicolor/{}/apps/{id}.{extension}",
            if extension == "svg" {
                "scalable"
            } else {
                "256x256"
            }
        )),
    };
    let old_launcher = existing(&result.launcher)?;
    let old_icon = existing(&result.icon)?;
    let other = options.data_dir.join(if extension == "svg" {
        format!("icons/hicolor/256x256/apps/{id}.png")
    } else {
        format!("icons/hicolor/scalable/apps/{id}.svg")
    });
    if fs::symlink_metadata(other).is_ok() {
        return Err(error("icon format change/collision: keep the previous SVG/PNG format or use a different app ID"));
    }
    if old_launcher.is_some() || old_icon.is_some() {
        let managed = old_launcher
            .as_deref()
            .and_then(|v| std::str::from_utf8(v).ok())
            .is_some_and(|s| {
                s.lines().any(|l| l == "X-Volt-Managed=true")
                    && s.lines().any(|l| l == format!("StartupWMClass={id}"))
            });
        if !options.replace || !managed {
            return Err(error("destination already exists: only a managed registration can be updated using --replace; nothing overwritten"));
        }
    }
    let desktop = format!("[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={id}\nTerminal=false\nCategories=Utility;\nStartupNotify=false\nStartupWMClass={id}\nX-Volt-Managed=true\n", value_escape(&options.name), exec_argument(path_text(&binary)?));
    publish(&result.icon, &image, old_icon.is_some())?;
    if let Err(e) = publish(&result.launcher, desktop.as_bytes(), old_launcher.is_some()) {
        let rollback = if let Some(old) = old_icon {
            publish(&result.icon, &old, true)
        } else {
            fs::remove_file(&result.icon)
        };
        return Err(error(format!(
            "launcher registration failed: {e}; icon rollback: {rollback:?}"
        )));
    }
    Ok(result)
}
pub fn run(args: &[String]) -> io::Result<()> {
    if !cfg!(target_os = "linux") {
        return Err(error("install-linux is Linux-only; no files changed"));
    }
    let options = InstallOptions::parse(args)?;
    let result = install(&options)?;
    println!("Registered {}\nLauncher: {}\nIcon: {}\nExecutable stays at {}\nRestart the app; KDE/other desktops may refresh their application/icon cache asynchronously.", options.name, result.launcher.display(), result.icon.display(), options.binary.canonicalize()?.display());
    Ok(())
}
