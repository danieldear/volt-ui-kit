//! The Linux desktop's reduce-motion preference, read once at startup.
//!
//! KDE keeps it in `kdeglobals` (`[KDE] AnimationDurationFactor=0` is
//! "Instant"); reading that file is cheap enough for the UI thread. GNOME keeps
//! it in the `org.gnome.desktop.interface enable-animations` setting, read with
//! a `gsettings` subprocess that the host runs on a background thread. Either
//! one can only turn animation off.
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

/// True when a `kdeglobals` file in the KDE config cascade sets the animation
/// speed to instant. The user's file overrides the system ones.
#[cfg(target_os = "linux")]
pub(super) fn kde_reduced_motion() -> bool {
    let paths = kdeglobals_paths(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("XDG_CONFIG_DIRS").as_deref(),
    );
    first_kde_setting(paths.iter().map(|path| {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)
            .ok()?
            .take(65_537)
            .read_to_string(&mut text)
            .ok()?;
        (text.len() <= 65_536).then_some(text)
    }))
}

/// True when GNOME's `enable-animations` is off. Runs a subprocess: call it
/// off the UI thread. A missing `gsettings` or schema means no preference.
#[cfg(target_os = "linux")]
pub(super) fn gnome_reduced_motion() -> bool {
    let mut command = std::process::Command::new("gsettings");
    command.args(["get", "org.gnome.desktop.interface", "enable-animations"]);
    bounded_output(command, std::time::Duration::from_secs(2))
        .is_some_and(|output| gnome_animations_off(&String::from_utf8_lossy(&output)))
}

/// Only read preferences belonging to the active desktop, not stale settings
/// left behind by another installed desktop environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Desktop {
    Kde,
    Gnome,
    Unknown,
}
#[cfg(target_os = "linux")]
pub(super) fn desktop() -> Desktop {
    desktop_from(
        std::env::var_os("XDG_CURRENT_DESKTOP").as_deref(),
        std::env::var_os("XDG_SESSION_DESKTOP").as_deref(),
    )
}
fn desktop_from(current: Option<&OsStr>, session: Option<&OsStr>) -> Desktop {
    for value in [current, session]
        .into_iter()
        .flatten()
        .filter_map(OsStr::to_str)
    {
        for name in value.split(':').map(str::trim) {
            if name.eq_ignore_ascii_case("KDE") {
                return Desktop::Kde;
            }
            if name.eq_ignore_ascii_case("GNOME") {
                return Desktop::Gnome;
            }
        }
    }
    Desktop::Unknown
}

/// A one-shot query with bounded time and output. A nonblocking local socket
/// captures stdout without a second reader thread or a pipe-capacity deadlock.
/// Any early return kills and reaps the child. No shell is used in production.
fn bounded_output(
    mut command: std::process::Command,
    timeout: std::time::Duration,
) -> Option<Vec<u8>> {
    use std::{
        io::Read,
        os::{fd::OwnedFd, unix::net::UnixStream},
        process::{Child, Stdio},
        time::{Duration, Instant},
    };
    struct Query(Child);
    impl Drop for Query {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let (mut reader, writer) = UnixStream::pair().ok()?;
    reader.set_nonblocking(true).ok()?;
    let stdout: OwnedFd = writer.into();
    let mut query = Query(
        command
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::from(stdout))
            .spawn()
            .ok()?,
    );
    // Command may retain its copy of the stdout descriptor until dropped.
    drop(command);
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    loop {
        let mut bytes = [0; 32];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => {
                    if output.len() + n > 64 {
                        return None;
                    }
                    output.extend_from_slice(&bytes[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return None,
            }
        }
        if let Some(status) = query.0.try_wait().ok()? {
            // The process may have exited between our first read and try_wait.
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(n) => {
                        if output.len() + n > 64 {
                            return None;
                        }
                        output.extend_from_slice(&bytes[..n]);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => return None,
                }
            }
            return status.success().then_some(output);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(
            Duration::from_millis(10).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

/// `$XDG_CONFIG_HOME/kdeglobals` (default `~/.config`), then each
/// `$XDG_CONFIG_DIRS` entry (default `/etc/xdg`), most important first.
/// Relative entries are ignored, as the XDG spec requires.
fn kdeglobals_paths(
    home: Option<&OsStr>,
    config_home: Option<&OsStr>,
    config_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let absolute = |value: &OsStr| {
        let path = Path::new(value);
        path.is_absolute().then(|| path.to_path_buf())
    };
    let user = config_home
        .and_then(absolute)
        .or_else(|| home.and_then(absolute).map(|home| home.join(".config")));
    let dirs = config_dirs
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or(OsStr::new("/etc/xdg"));
    user.into_iter()
        .chain(std::env::split_paths(dirs).filter_map(|dir| absolute(dir.as_os_str())))
        .map(|dir| dir.join("kdeglobals"))
        .collect()
}

/// The first file in the cascade that sets the factor decides.
fn first_kde_setting(files: impl IntoIterator<Item = Option<String>>) -> bool {
    files
        .into_iter()
        .flatten()
        .find_map(|text| kde_animations_off(&text))
        .unwrap_or(false)
}

/// `AnimationDurationFactor` from the `[KDE]` group of one kdeglobals file:
/// `Some(true)` for zero (instant), `Some(false)` for a positive factor, and
/// `None` when the file doesn't set a usable value. The last entry wins, as in
/// KConfig. Flags such as `[$i]` on the group or key are accepted.
fn kde_animations_off(text: &str) -> Option<bool> {
    let mut in_kde = false;
    let mut setting = None;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_kde = strip_kconfig_flags(line) == "[KDE]";
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !in_kde || strip_kconfig_flags(key.trim()) != "AnimationDurationFactor" {
            continue;
        }
        if let Ok(factor) = value.trim().parse::<f64>() {
            if factor.is_finite() {
                setting = Some(factor <= 0.);
            }
        }
    }
    setting
}

/// Remove trailing KConfig flag suffixes such as `[$i]` or `[$e]`.
fn strip_kconfig_flags(mut name: &str) -> &str {
    while let Some(start) = name.rfind("[$") {
        if !name.ends_with(']') {
            break;
        }
        name = name[..start].trim_end();
    }
    name
}

/// `gsettings get` prints a boolean as `true` or `false`.
fn gnome_animations_off(stdout: &str) -> bool {
    stdout.trim() == "false"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_desktop_selection_does_not_mix_gnome_and_kde_preferences() {
        let os = |s: &'static str| Some(OsStr::new(s));
        assert_eq!(desktop_from(os("KDE"), os("GNOME")), Desktop::Kde);
        assert_eq!(desktop_from(os("ubuntu:GNOME"), os("KDE")), Desktop::Gnome);
        assert_eq!(
            desktop_from(os("GNOME-Classic:GNOME"), None),
            Desktop::Gnome
        );
        assert_eq!(desktop_from(None, os("kde")), Desktop::Kde);
        assert_eq!(desktop_from(os("COSMIC"), None), Desktop::Unknown);
        assert_eq!(desktop_from(None, None), Desktop::Unknown);
    }

    fn shell(script: &str) -> std::process::Command {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn settings_queries_require_success_and_bounded_output() {
        let timeout = std::time::Duration::from_secs(1);
        assert_eq!(
            bounded_output(shell("printf 'false\\n'"), timeout),
            Some(b"false\n".to_vec())
        );
        assert!(bounded_output(shell("printf false; exit 1"), timeout).is_none());
        assert!(bounded_output(shell("printf '%100s' x"), timeout).is_none());
        assert!(bounded_output(
            std::process::Command::new("/nonexistent/volt-settings-qa"),
            timeout
        )
        .is_none());
    }

    #[test]
    fn stuck_settings_queries_are_killed_and_reaped() {
        let started = std::time::Instant::now();
        assert!(
            bounded_output(shell("exec sleep 5"), std::time::Duration::from_millis(30)).is_none()
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn kde_zero_factor_turns_animation_off() {
        let text = "[General]\nColorScheme=BreezeDark\n\n[KDE]\nAnimationDurationFactor=0\n";
        assert_eq!(kde_animations_off(text), Some(true));
        assert_eq!(
            kde_animations_off("[KDE]\nAnimationDurationFactor = 0.0\n"),
            Some(true)
        );
    }

    #[test]
    fn kde_positive_factor_or_missing_key_keeps_animation() {
        assert_eq!(
            kde_animations_off("[KDE]\nAnimationDurationFactor=0.5\n"),
            Some(false)
        );
        assert_eq!(kde_animations_off("[KDE]\nSingleClick=false\n"), None);
        assert_eq!(kde_animations_off(""), None);
        assert_eq!(
            kde_animations_off("[KDE]\nAnimationDurationFactor=fast\n"),
            None
        );
        assert_eq!(
            kde_animations_off("[KDE]\nAnimationDurationFactor=NaN\n"),
            None
        );
    }

    #[test]
    fn kde_key_only_counts_in_the_kde_group() {
        assert_eq!(
            kde_animations_off("[General]\nAnimationDurationFactor=0\n"),
            None
        );
        // `[KDE][Sub]` is a subgroup, not `[KDE]`.
        assert_eq!(
            kde_animations_off("[KDE][Sub]\nAnimationDurationFactor=0\n"),
            None
        );
        assert_eq!(
            kde_animations_off("[KDE]\n[Other]\nAnimationDurationFactor=0\n"),
            None
        );
        // A localized entry is a different key.
        assert_eq!(
            kde_animations_off("[KDE]\nAnimationDurationFactor[de]=0\n"),
            None
        );
    }

    #[test]
    fn kde_flags_comments_and_repeats_follow_kconfig() {
        assert_eq!(
            kde_animations_off("[KDE][$i]\nAnimationDurationFactor[$i]=0\n"),
            Some(true)
        );
        assert_eq!(
            kde_animations_off("[KDE]\n# AnimationDurationFactor=1\nAnimationDurationFactor=0\n"),
            Some(true)
        );
        let repeated =
            "[KDE]\nAnimationDurationFactor=0\n[General]\n[KDE]\nAnimationDurationFactor=1\n";
        assert_eq!(kde_animations_off(repeated), Some(false));
        assert_eq!(
            kde_animations_off("\u{feff}[KDE]\r\nAnimationDurationFactor=0\r\n"),
            Some(true)
        );
    }

    #[test]
    fn user_kdeglobals_overrides_system_defaults() {
        let off = Some("[KDE]\nAnimationDurationFactor=0\n".to_string());
        let on = Some("[KDE]\nAnimationDurationFactor=1\n".to_string());
        let unset = Some("[General]\n".to_string());
        assert!(first_kde_setting([None, off.clone()]));
        assert!(first_kde_setting([unset.clone(), off.clone()]));
        assert!(!first_kde_setting([on, off.clone()]));
        assert!(first_kde_setting([off, None]));
        assert!(!first_kde_setting([unset, None]));
        assert!(!first_kde_setting(Vec::new()));
    }

    #[test]
    fn kdeglobals_cascade_follows_xdg_base_directories() {
        let os = |s: &'static str| Some(OsStr::new(s));
        assert_eq!(
            kdeglobals_paths(os("/home/a"), None, None),
            [
                PathBuf::from("/home/a/.config/kdeglobals"),
                PathBuf::from("/etc/xdg/kdeglobals"),
            ]
        );
        assert_eq!(
            kdeglobals_paths(os("/home/a"), os("/cfg"), os("")),
            [
                PathBuf::from("/cfg/kdeglobals"),
                PathBuf::from("/etc/xdg/kdeglobals"),
            ]
        );
        assert_eq!(
            kdeglobals_paths(
                os("/home/a"),
                os("relative"),
                os("/etc/xdg/kde:rel:/etc/xdg")
            ),
            [
                PathBuf::from("/home/a/.config/kdeglobals"),
                PathBuf::from("/etc/xdg/kde/kdeglobals"),
                PathBuf::from("/etc/xdg/kdeglobals"),
            ]
        );
        assert!(kdeglobals_paths(None, None, os("relative")).is_empty());
    }

    #[test]
    fn gnome_only_false_turns_animation_off() {
        assert!(gnome_animations_off("false\n"));
        assert!(!gnome_animations_off("true\n"));
        assert!(!gnome_animations_off(""));
        assert!(!gnome_animations_off(
            "No such schema “org.gnome.desktop.interface”\n"
        ));
    }
}
