//! Opt-in iOS scaffolding. Desktop templates remain on their existing path.
use super::{error, fs, io, Path};

pub fn resolve<'a>(kind: &'a str, platform: Option<&str>, desktop: bool) -> io::Result<&'a str> {
    let kind=match platform {
        None=>kind,
        Some("desktop") if kind!="ios"=>kind,
        Some("ios") if ["minimal","navigation","ios"].contains(&kind)=>"ios",
        Some("android")=>return Err(error("Android has no host/template yet. The common Rust application layer is platform-neutral; Android runtime support is not implemented.")),
        _=>return Err(error("unsupported platform/template combination; use --platform ios with minimal/navigation, or an existing desktop template")),
    };
    if kind == "ios" && desktop {
        return Err(error(
            "--desktop integrations cannot be combined with an iOS template",
        ));
    }
    Ok(kind)
}
fn quoted(value: &str) -> io::Result<String> {
    serde_json::to_string(value).map_err(|e| error(e.to_string()))
}
pub fn scaffold(destination: &Path, kit: &Path) -> io::Result<()> {
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("invalid project name"))?;
    if name.len() > 64
        || name.is_empty()
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(error(
            "iOS project name must be 1–64 ASCII letters/digits/-/_, starting with a letter",
        ));
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(error("destination already exists; nothing overwritten"));
    }
    let kit = kit.canonicalize()?;
    let manifest = fs::read_to_string(kit.join("Cargo.toml"))?;
    if !manifest.contains("name = \"volt-ui-kit\"") {
        return Err(error("--kit-path is not Volt UI Kit"));
    }
    let host = kit.join("crates/volt-ui-kit-mobile");
    for file in [
        "Cargo.toml",
        "apple/VoltUIHost.swift",
        "apple/VoltBridge.h",
        "apple/build-rust.sh",
    ] {
        if !host.join(file).is_file() {
            return Err(error(format!(
                "toolkit is missing the reusable mobile host: {file}"
            )));
        }
    }
    let host = host
        .to_str()
        .ok_or_else(|| error("host path must be UTF-8"))?;
    // Xcode expands $() in build settings. Reject ambiguous expansion/control
    // characters rather than silently generating a different path. Spaces work.
    if host.chars().any(|c| c.is_control() || c == '$') {
        return Err(error(
            "iOS toolkit paths cannot contain control characters or $ (Xcode setting expansion)",
        ));
    }
    let host_dependency = quoted(host)?;
    let apple = format!("{host}/apple");
    let apple_literal = quoted(&apple)?;
    let header_literal = quoted(&format!("{apple}/VoltBridge.h"))?;
    let project = include_str!("ios/project.pbxproj.in")
        .replace("\"__VOLT_HOST_APPLE__/VoltBridge.h\"", &header_literal)
        .replace("\"__VOLT_HOST_APPLE__\"", &apple_literal)
        .replace(
            "__BUNDLE_ID__",
            &format!("dev.volt.{}", name.to_ascii_lowercase().replace('_', "-")),
        );
    if project.contains("__VOLT_HOST") {
        return Err(error("incomplete iOS project template"));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        fs::create_dir_all(destination.join("src"))?;
        fs::create_dir_all(destination.join("ios/App"))?;
        fs::create_dir_all(destination.join("ios/Tests"))?;
        fs::create_dir_all(destination.join("ios/VoltApp.xcodeproj/xcshareddata/xcschemes"))?;
        fs::write(destination.join("Cargo.toml"),format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"1.89\"\npublish = false\n\n[lib]\ncrate-type = [\"staticlib\", \"rlib\"]\n\n[dependencies]\nvolt-ui-kit-mobile = {{ path = {host_dependency} }}\n\n[profile.release]\nopt-level = 2\n"))?;
        fs::write(
            destination.join("src/lib.rs"),
            include_str!("ios/app.rs.in").replace("__APP_NAME__", name),
        )?;
        fs::write(
            destination.join("ios/VoltApp.xcodeproj/project.pbxproj"),
            project,
        )?;
        fs::write(
            destination.join("ios/VoltApp.xcodeproj/xcshareddata/xcschemes/VoltApp.xcscheme"),
            include_str!("ios/scheme.xcscheme.in"),
        )?;
        fs::write(
            destination.join("ios/App/Info.plist"),
            include_str!("ios/App/Info.plist").replace("__APP_NAME__", name),
        )?;
        fs::write(
            destination.join("ios/Tests/SmokeTests.swift"),
            include_str!("ios/Tests/SmokeTests.swift"),
        )?;
        fs::write(
            destination.join("ios/build-rust.sh"),
            include_str!("ios/build-rust.sh"),
        )?;
        fs::write(
            destination.join(".gitignore"),
            "/target/\n.DS_Store\n**/xcuserdata/\n",
        )?;
        fs::write(
            destination.join("README.md"),
            format!(
                r#"# {name}

Experimental Rust-native Volt iOS app. Edit `src/lib.rs`; do not copy or write
Swift/UIKit glue. The Xcode project references the canonical reusable host in
`volt-ui-kit-mobile`. There is no per-app Swift application source.

On an Apple-silicon Mac with Xcode:

```sh
rustup target add aarch64-apple-ios-sim aarch64-apple-ios
open ios/VoltApp.xcodeproj
```

Choose **VoltApp** and an iPhone Simulator, then Run. Xcode builds Rust for the
selected SDK automatically; XcodeGen is not required. First build resolves
Cargo.lock; commit that lockfile. Later builds use `--locked`. A normal
`cargo build` builds a library, not a desktop executable; `cargo volt dev` is
currently desktop-only.

```sh
cargo test --locked
xcodebuild -project ios/VoltApp.xcodeproj -scheme VoltApp \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' test
```

Use an installed Simulator name. Build paths with spaces are supported. Local
`--kit-path` dependencies and the referenced host sources must remain available;
this template does not vendor or duplicate toolkit code. Recreate/update the
project references if you move the toolkit checkout. Xcode expands build-setting
variables: toolkit paths containing `$` or control characters are rejected.

The generated app is unsigned by default. For a device, enable Code Signing
Allowed, choose your Development Team and a unique bundle ID in Xcode. Signing,
App Store release, physical GPU/energy testing and complete mobile accessibility
are separate acceptance gates. No permission or network request is made by the
app itself. Settings are session-only.

The template explicitly allows software drawing + Metal presentation when the
Simulator lacks Vello compute/indirect support; compatible hardware uses GPU
drawing. Inspect the renderer diagnostic instead of assuming GPU rasterization.
All rendering/theme/input behavior is owned by the Rust application/Volt host.

Android is not implemented yet. Its future Activity/native-window/IME adapter
can reuse the platform-neutral Rust Application/PlatformEvent layer, but this
Xcode project and UIKit code are iOS-only.
"#
            ),
        )?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(destination);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_selection_is_explicit_and_desktop_defaults_do_not_change() {
        assert_eq!(resolve("minimal", None, false).unwrap(), "minimal");
        assert_eq!(
            resolve("navigation", Some("desktop"), true).unwrap(),
            "navigation"
        );
        assert_eq!(resolve("minimal", Some("ios"), false).unwrap(), "ios");
        assert_eq!(resolve("ios", None, false).unwrap(), "ios");
        for (kind, p, d) in [
            ("mail", Some("ios"), false),
            ("ios", Some("desktop"), false),
            ("ios", None, true),
            ("minimal", Some("android"), false),
        ] {
            assert!(resolve(kind, p, d).is_err());
        }
    }
    #[test]
    fn ios_project_references_shared_sources_not_copied_glue() {
        let root = std::env::temp_dir().join(format!("volt-ios-scaffold-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let dest = root.join("my-app");
        scaffold(&dest, Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let project =
            fs::read_to_string(dest.join("ios/VoltApp.xcodeproj/project.pbxproj")).unwrap();
        assert!(project.contains("volt-ui-kit-mobile/apple"));
        assert!(!project.contains("__VOLT_HOST"));
        assert!(fs::read_to_string(dest.join("Cargo.toml"))
            .unwrap()
            .contains("staticlib"));
        assert!(fs::read_to_string(dest.join("src/lib.rs"))
            .unwrap()
            .contains("export_ios_app!"));
        assert!(!dest.join("ios/App/VoltUIHost.swift").exists());
        fs::write(dest.join("sentinel"), "keep").unwrap();
        assert!(scaffold(&dest, Path::new(env!("CARGO_MANIFEST_DIR"))).is_err());
        assert_eq!(fs::read_to_string(dest.join("sentinel")).unwrap(), "keep");
        assert!(scaffold(
            &root.join("bad;name"),
            Path::new(env!("CARGO_MANIFEST_DIR"))
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_host_and_symlink_destination_never_create_or_overwrite() {
        let root =
            std::env::temp_dir().join(format!("volt-ios-missing-host-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let kit = root.join("kit");
        fs::create_dir(&kit).unwrap();
        fs::write(
            kit.join("Cargo.toml"),
            r#"[package]
name = "volt-ui-kit"
"#,
        )
        .unwrap();
        let dest = root.join("my-app");
        assert!(scaffold(&dest, &kit).is_err());
        assert!(!dest.exists());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&kit, &dest).unwrap();
            assert!(scaffold(&dest, Path::new(env!("CARGO_MANIFEST_DIR"))).is_err());
            assert!(fs::symlink_metadata(&dest)
                .unwrap()
                .file_type()
                .is_symlink());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
