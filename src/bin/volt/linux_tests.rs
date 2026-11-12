use super::linux::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "volt-linux-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn options(&self) -> InstallOptions {
        let binary = self.0.join(if cfg!(windows) {
            "My app executable"
        } else {
            "My app $ ` \\\" executable"
        });
        fs::write(&binary, b"test executable").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let icon = self.0.join("icon.svg");
        fs::write(&icon, DEFAULT_ICON).unwrap();
        InstallOptions::parse(
            &[
                "--binary",
                binary.to_str().unwrap(),
                "--app-id",
                "com.example.test",
                "--name",
                "Example app",
                "--icon",
                icon.to_str().unwrap(),
                "--data-dir",
                self.0.join("data").to_str().unwrap(),
            ]
            .map(str::to_owned),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn install_matches_identity_and_copies_icon_without_copying_binary() {
    let f = Fixture::new();
    let options = f.options();
    let result = install(&options).unwrap();
    let desktop = fs::read_to_string(&result.launcher).unwrap();
    assert_eq!(
        result.launcher.file_name().unwrap(),
        "com.example.test.desktop"
    );
    assert!(desktop.contains("Icon=com.example.test\n"));
    assert!(desktop.contains("StartupWMClass=com.example.test\n"));
    assert!(desktop.contains("Terminal=false\n"));
    assert!(desktop.contains("X-Volt-Managed=true\n"));
    assert!(!desktop.contains("sh -c"));
    assert!(!desktop.contains("%%"));
    #[cfg(unix)]
    assert!(desktop.contains("\\\\$"));
    assert_eq!(fs::read(&result.icon).unwrap(), DEFAULT_ICON.as_bytes());
    assert!(options.binary.exists());
    assert!(!options.data_dir.join("bin").exists());
}
#[test]
fn validation_precedes_creating_files() {
    let f = Fixture::new();
    for id in [
        "bad",
        "com.example/escape",
        "com.example\nExec=bad",
        "com..app",
        "com.0app",
    ] {
        let mut options = f.options();
        options.app_id = id.into();
        assert!(install(&options).is_err());
        assert!(!options.data_dir.exists());
    }
    let mut options = f.options();
    options.name = "bad\nExec=bad".into();
    assert!(install(&options).is_err());
    assert!(!options.data_dir.exists());
    options = f.options();
    options.data_dir = "relative".into();
    assert!(install(&options).is_err());
    options = f.options();
    options.icon = f.0.join("not-an-icon.svg");
    fs::write(&options.icon, b"invalid").unwrap();
    assert!(install(&options).is_err());
    assert!(!options.data_dir.exists());
}
#[test]
fn installs_never_overwrite_without_explicit_replace() {
    let f = Fixture::new();
    let mut options = f.options();
    let result = install(&options).unwrap();
    let old = fs::read(&result.launcher).unwrap();
    options.name = "New name".into();
    assert!(install(&options).is_err());
    assert_eq!(fs::read(&result.launcher).unwrap(), old);
    options.replace = true;
    install(&options).unwrap();
    assert!(fs::read_to_string(result.launcher)
        .unwrap()
        .contains("Name=New\\sname\n"));
}
#[test]
fn replace_refuses_unmanaged_entries_and_collisions_without_partial_files() {
    let f = Fixture::new();
    let mut options = f.options();
    let launcher = options
        .data_dir
        .join("applications/com.example.test.desktop");
    fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    fs::write(&launcher, "[Desktop Entry]\nName=Someone else\n").unwrap();
    options.replace = true;
    assert!(install(&options).is_err());
    assert!(!options.data_dir.join("icons").exists());
    fs::remove_file(&launcher).unwrap();
    fs::create_dir(&launcher).unwrap();
    assert!(install(&options).is_err());
    assert!(!options.data_dir.join("icons").exists());
}
#[cfg(unix)]
#[test]
fn installer_refuses_destination_symlinks_and_nonexecutable_binaries() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let options = f.options();
    let launcher = options
        .data_dir
        .join("applications/com.example.test.desktop");
    fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    let victim = f.0.join("victim");
    fs::write(&victim, "unchanged").unwrap();
    symlink(&victim, &launcher).unwrap();
    assert!(install(&options).is_err());
    assert_eq!(fs::read_to_string(victim).unwrap(), "unchanged");
    fs::remove_file(launcher).unwrap();
    fs::set_permissions(&options.binary, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(install(&options).is_err());
}
#[test]
fn parser_rejects_duplicate_unknown_and_missing_options() {
    for args in [
        vec![],
        vec!["--binary"],
        vec!["--unknown", "value"],
        vec!["--name", "one", "--name", "two"],
    ] {
        assert!(
            InstallOptions::parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>())
                .is_err()
        );
    }
}

#[test]
fn png_dimensions_and_icon_limits_are_checked_before_installation() {
    let f = Fixture::new();
    let mut options = f.options();
    options.icon = f.0.join("icon.png");
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1]);
    fs::write(&options.icon, &png).unwrap();
    assert!(install(&options).is_err());
    assert!(!options.data_dir.exists());
    png[16..24].copy_from_slice(&[0, 0, 1, 0, 0, 0, 1, 0]);
    fs::write(&options.icon, &png).unwrap();
    let result = install(&options).unwrap();
    assert!(result.icon.ends_with("256x256/apps/com.example.test.png"));
    let mut oversized = DEFAULT_ICON.as_bytes().to_vec();
    oversized.resize(4 * 1024 * 1024 + 1, b' ');
    let other = Fixture::new();
    let options = other.options();
    fs::write(&options.icon, oversized).unwrap();
    assert!(install(&options).is_err());
    assert!(!options.data_dir.exists());
}

#[test]
fn nonportable_exec_paths_fail_before_registration() {
    let f = Fixture::new();
    for name in ["percent % path", "equals = path"] {
        let mut options = f.options();
        let path = f.0.join(name);
        fs::copy(&options.binary, &path).unwrap();
        options.binary = path;
        assert!(install(&options).is_err());
        assert!(!options.data_dir.exists());
    }
}
