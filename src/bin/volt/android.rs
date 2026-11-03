//! Opt-in Android scaffolding. OS glue lives once in the mobile host crate.
use super::{error, fs, io, Path};
pub fn scaffold(destination: &Path, kit: &Path) -> io::Result<()> {
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| error("invalid project name"))?;
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(error(
            "Android project name must be 1–64 ASCII letters/digits/-/_, starting with a letter",
        ));
    }
    if fs::symlink_metadata(destination).is_ok() {
        return Err(error("destination exists; nothing overwritten"));
    }
    let kit = kit.canonicalize()?;
    if !fs::read_to_string(kit.join("Cargo.toml"))?.contains("name = \"volt-ui-kit\"") {
        return Err(error("--kit-path is not Volt UI Kit"));
    }
    let host = kit.join("crates/volt-ui-kit-mobile");
    let shared = [
        "android/NOTICE.md",
        "android/gradlew",
        "android/gradlew.bat",
        "android/gradle/wrapper/gradle-wrapper.jar",
        "android/gradle/wrapper/gradle-wrapper.properties",
    ];
    for file in shared.iter().copied().chain([
        "Cargo.toml",
        "android/java/dev/volt/host/VoltActivity.java",
        "android/build-rust.py",
    ]) {
        if !host.join(file).is_file() {
            return Err(error(format!("missing Android host file: {file}")));
        }
    }
    let host = host
        .to_str()
        .ok_or_else(|| error("host path must be UTF-8"))?;
    if host.chars().any(|c| c.is_control() || c == '$') {
        return Err(error(
            "Android toolkit paths cannot contain control characters or $",
        ));
    }
    let host_literal = serde_json::to_string(host).map_err(|e| error(e.to_string()))?;
    fs::create_dir(destination)?;
    let result = (|| {
        fs::create_dir_all(destination.join("src"))?;
        fs::create_dir_all(destination.join("android/gradle/wrapper"))?;
        fs::write(destination.join("Cargo.toml"),format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"1.89\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n\n[dependencies]\nvolt-ui-kit-mobile = {{ path = {host_literal} }}\n\n[profile.release]\nopt-level = 2\n"))?;
        let source = include_str!("mobile-app.rs.in")
            .replace("__APP_NAME__", name)
            .replace("__VOLT_EXPORT__", "export_android_app");
        fs::write(destination.join("src/lib.rs"), source)?;
        let bundle = format!(
            "dev.volt.apps.{}",
            name.to_ascii_lowercase().replace('-', "_")
        );
        fs::write(
            destination.join("android/build.gradle"),
            include_str!("android/build.gradle.in")
                .replace("__VOLT_HOST__", &host_literal)
                .replace("__BUNDLE_ID__", &bundle),
        )?;
        fs::write(
            destination.join("android/AndroidManifest.xml"),
            include_str!("android/AndroidManifest.xml.in").replace("__APP_NAME__", name),
        )?;
        fs::write(destination.join("android/settings.gradle"),"pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositories { google(); mavenCentral() } }\nrootProject.name = 'VoltApp'\n")?;
        fs::write(
            destination.join("android/gradle.properties"),
            "org.gradle.jvmargs=-Xmx1536m\norg.gradle.daemon=false\nandroid.useAndroidX=false\n",
        )?;
        for path in shared {
            fs::copy(Path::new(host).join(path), destination.join(path))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                destination.join("android/gradlew"),
                fs::Permissions::from_mode(0o755),
            )?;
        }
        fs::write(
            destination.join(".gitignore"),
            "/target/\n/android/.gradle/\n/android/build/\n/android/local.properties\n.DS_Store\n",
        )?;
        fs::write(destination.join("README.md"),format!("# {name}\n\nExperimental Rust-native Android app. Edit src/lib.rs. The canonical shared\nVoltActivity supplies the SurfaceView/Vulkan lifecycle, native single-line\nkeyboard and basic accessibility proxies, not your application screens.\nNo per-app Java/Kotlin/Compose or WebView is needed.\n\nRequires JDK 17, Python 3, Cargo 1.89+, Android SDK 36 and NDK 29.0.13599879.\nInstall aarch64-linux-android with rustup. Set ANDROID_HOME to your SDK; put\nCargo on PATH. The checked-in Gradle 8.13 wrapper verifies its distribution\nSHA-256. Dependencies resolve on the first build; commit Cargo.lock.\n\n```sh\nrustup target add aarch64-linux-android\ncd android\n./gradlew assembleDebug\nadb install -r build/outputs/apk/debug/VoltApp-debug.apk\nadb shell am start -n {bundle}/dev.volt.host.VoltActivity\n```\n\nFor an x86_64 emulator, install x86_64-linux-android and pass -PvoltAbi=x86_64.\nThe starter is API 30+, compile/target SDK 36; this is not complete device\ncoverage. Debug signing is local/test-only; release signing is developer-owned.\n\nApp state is session-only. Rotation and temporary surface loss preserve it;\nprocess death resets it. No app permission, network request or telemetry is\nadded. Native secure fields remain read-only; full IME, TalkBack, kinetic\ngestures and physical-device GPU/energy acceptance are pending. Diagnostics\nreport Vello GPU drawing versus explicit software drawing/Vulkan presentation.\n\nThe project references the local mobile host; moving the toolkit requires\nupdating Cargo and Gradle paths. Desktop and iOS APIs remain separate.\n"))?;
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
    fn android_project_uses_shared_host_and_never_overwrites() {
        let root =
            std::env::temp_dir().join(format!("volt-android-scaffold-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let dest = root.join("rust-phone");
        scaffold(&dest, Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let source = fs::read_to_string(dest.join("src/lib.rs")).unwrap();
        assert!(source.contains("export_android_app!"));
        assert!(source.contains("back_with"));
        let project = fs::read_to_string(dest.join("android/build.gradle")).unwrap();
        assert!(project.contains("volt-ui-kit-mobile"));
        assert!(!dest.join("android/src").exists());
        let wrapper =
            fs::read_to_string(dest.join("android/gradle/wrapper/gradle-wrapper.properties"))
                .unwrap();
        assert!(wrapper.contains("distributionSha256Sum="));
        fs::write(dest.join("sentinel"), "keep").unwrap();
        assert!(scaffold(&dest, Path::new(env!("CARGO_MANIFEST_DIR"))).is_err());
        assert_eq!(fs::read_to_string(dest.join("sentinel")).unwrap(), "keep");
        fs::remove_dir_all(root).unwrap();
    }
}
