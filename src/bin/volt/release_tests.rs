use super::release::*;
use serde_json::json;
use std::path::{Path, PathBuf};

fn metadata(bundle: serde_json::Value) -> serde_json::Value {
    json!({
        "packages": [{
            "name": "notes",
            "version": "1.4.0",
            "id": "notes 1.4.0",
            "manifest_path": "/work/notes/Cargo.toml",
            "targets": [
                {"name": "notes", "kind": ["bin"]},
                {"name": "notes-preview", "kind": ["bin"]},
                {"name": "notes", "kind": ["lib"]}
            ],
            "metadata": {"volt": {"bundle": bundle}}
        }],
        "target_directory": "/work/target",
        "resolve": null
    })
}

fn markdown_bundle() -> serde_json::Value {
    json!({
        "name": "Notes Pro",
        "identifier": "dev.example.notes",
        "icon": "assets/AppIcon.icns",
        "category": "public.app-category.productivity",
        "copyright": "© 2026 Example",
        "document-types": [
            {"name": "Markdown", "role": "Editor", "rank": "Default",
             "content-types": ["net.daringfireball.markdown"], "icon": "AppIcon.icns"},
            {"name": "Markdown", "role": "Editor", "extensions": ["mdown", "mkd"]}
        ],
        "imported-types": [
            {"identifier": "net.daringfireball.markdown", "description": "Markdown",
             "conforms-to": ["public.plain-text"], "extensions": ["md"], "mime-type": "text/markdown"}
        ]
    })
}

#[test]
fn metadata_names_the_package_binary_and_bundle() {
    let m = manifest_from_json(&metadata(markdown_bundle()), None).unwrap();
    assert_eq!(
        (m.package.as_str(), m.bin.as_str(), m.version.as_str()),
        ("notes", "notes", "1.4.0")
    );
    assert_eq!(m.dir, Path::new("/work/notes"));
    assert_eq!(out_dir(&m), Path::new("/work/target/release/bundle"));
    let missing = manifest_from_json(&metadata(json!(null)), None).unwrap_err();
    assert!(missing.to_string().contains("package.metadata.volt.bundle"));
    let mut wrong = markdown_bundle();
    wrong["binary"] = json!("other");
    assert!(manifest_from_json(&metadata(wrong), None).is_err());
}

#[test]
fn document_types_and_imported_types_reach_the_info_plist() {
    let m = manifest_from_json(&metadata(markdown_bundle()), None).unwrap();
    let o = options_from_manifest(
        &m,
        PathBuf::from("/bin/notes"),
        PathBuf::from("/out/Notes Pro.app"),
    )
    .unwrap();
    assert_eq!(o.name, "Notes Pro");
    assert_eq!(o.version, "1.4.0");
    assert_eq!(
        o.icon.as_deref(),
        Some(Path::new("/work/notes/assets/AppIcon.icns"))
    );
    assert_eq!(
        o.document_types[1].rank, "Default",
        "rank defaults to Default"
    );
    assert!(o.strip, "stripped by default");
    for (value, strip) in [
        (json!(false), Some(false)),
        (json!(true), Some(true)),
        (json!("yes"), None),
    ] {
        let mut bundle = markdown_bundle();
        bundle["strip"] = value;
        let m = manifest_from_json(&metadata(bundle), None).unwrap();
        let o = options_from_manifest(&m, "/bin/notes".into(), "/out/N.app".into());
        assert_eq!(o.ok().map(|o| o.strip), strip);
    }
    let xml = o.plist();
    for needle in [
        "<key>CFBundleDocumentTypes</key>",
        "<key>LSItemContentTypes</key><array><string>net.daringfireball.markdown</string></array>",
        "<key>CFBundleTypeExtensions</key><array><string>mdown</string><string>mkd</string></array>",
        "<key>UTImportedTypeDeclarations</key>",
        "<key>public.mime-type</key><string>text/markdown</string>",
        "<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>",
        "© 2026 Example",
    ] {
        assert!(xml.contains(needle), "missing {needle}\n{xml}");
    }
}

fn steps(o: &ReleaseOptions, identity: Option<&str>, profile: Option<&str>) -> Vec<String> {
    let m = manifest_from_json(&metadata(markdown_bundle()), None).unwrap();
    plan(&m, o, "Notes Pro", identity, profile)
        .into_iter()
        .map(|s| match s {
            Step::Run { program, args, .. } => {
                format!("{} {}", program.rsplit('/').next().unwrap(), args.join(" "))
            }
            Step::Bundle => "bundle".into(),
            Step::Remove(p) => format!("remove {}", p.display()),
            Step::Stage { .. } => "stage".into(),
        })
        .collect()
}

#[test]
fn bundling_builds_then_assembles_without_signing_steps() {
    let s = steps(&ReleaseOptions::parse(&[], false).unwrap(), None, None);
    assert_eq!(s.len(), 3);
    assert!(s[0]
        .starts_with("cargo build --release --bin notes --manifest-path /work/notes/Cargo.toml"));
    assert_eq!(s[1], "remove /work/target/release/bundle/Notes Pro.app");
    assert_eq!(s[2], "bundle");
}

#[test]
fn a_release_signs_notarizes_staples_and_ships_a_disk_image() {
    let args: Vec<String> = ["--universal"].map(String::from).to_vec();
    let o = ReleaseOptions::parse(&args, true).unwrap();
    let s = steps(&o, Some("ABC123"), Some("notes-profile"));
    let joined = s.join("\n");
    let order = [
        "--target aarch64-apple-darwin",
        "--target x86_64-apple-darwin",
        "lipo -create",
        "bundle",
        "codesign --force --options runtime --timestamp --sign ABC123",
        "codesign --verify --strict --deep",
        "ditto -c -k --sequesterRsrc --keepParent",
        "notarytool submit /work/target/release/bundle/Notes-Pro.zip --keychain-profile notes-profile --wait",
        "stapler staple /work/target/release/bundle/Notes Pro.app",
        "spctl --assess --type execute",
        "stage",
        "hdiutil create -volname Notes Pro",
        "codesign --force --timestamp --sign ABC123 /work/target/release/bundle/Notes-Pro-1.4.0.dmg",
        "notarytool submit /work/target/release/bundle/Notes-Pro-1.4.0.dmg",
        "stapler staple /work/target/release/bundle/Notes-Pro-1.4.0.dmg",
        "spctl --assess --type open",
    ];
    let mut at = 0;
    for needle in order {
        let found = joined[at..]
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} missing or out of order in\n{joined}"));
        at += found + needle.len();
    }
    // Without notarization or an image there is nothing to submit or stage.
    let args: Vec<String> = ["--no-notarize", "--no-dmg"].map(String::from).to_vec();
    let s = steps(
        &ReleaseOptions::parse(&args, true).unwrap(),
        Some("ABC123"),
        None,
    );
    assert!(s
        .iter()
        .all(|s| !s.contains("notarytool") && !s.contains("hdiutil")));
    assert!(s.iter().any(|s| s.contains("--options runtime")));
    assert!(
        ReleaseOptions::parse(&["--dry-run".into()], false).is_err(),
        "release-only flags"
    );
}

#[test]
fn identities_and_notary_results_are_read_strictly() {
    let listing = "  1) 21D5 \"Apple Development: A (X)\"\n  2) C5FE \"Developer ID Application: A (TEAM)\"\n     2 valid identities found";
    assert_eq!(developer_id(listing).unwrap(), "C5FE");
    assert!(developer_id("  1) 21D5 \"Apple Development: A (X)\"").is_err());
    let two = format!("{listing}\n  3) D00D \"Developer ID Application: B (T2)\"");
    assert!(developer_id(&two).is_err());
    assert!(accepted(r#"{"id":"1","status":"Accepted"}"#).is_ok());
    let invalid = accepted(r#"{"id":"abc","status":"Invalid"}"#)
        .unwrap_err()
        .to_string();
    assert!(invalid.contains("Invalid") && invalid.contains("notarytool log abc"));
    assert!(accepted("not json").is_err());
}

#[test]
fn app_extensions_are_built_merged_and_signed_before_the_app() {
    let mut bundle = markdown_bundle();
    bundle["extensions"] = json!([{
        "name": "Notes Preview",
        "identifier": "dev.example.notes.preview",
        "binary": "notes-preview",
        "point": "com.apple.quicklook.preview",
        "principal-class": "NotesPreviewProvider",
        "attributes": {"QLIsDataBasedPreview": true}
    }]);
    let m = manifest_from_json(&metadata(bundle.clone()), None).unwrap();
    let o = options_from_manifest(&m, "/out/notes-universal".into(), "/out/N.app".into()).unwrap();
    assert_eq!(
        o.extensions[0].binary,
        Path::new("/out/notes-preview-universal")
    );
    let o = options_from_manifest(&m, "/t/release/notes".into(), "/out/N.app".into()).unwrap();
    assert_eq!(
        o.extensions[0].binary,
        Path::new("/t/release/notes-preview")
    );
    let args: Vec<String> = ["--universal"].map(String::from).to_vec();
    let release = ReleaseOptions::parse(&args, true).unwrap();
    let joined: Vec<String> = plan(&m, &release, "Notes Pro", Some("ABC123"), None)
        .into_iter()
        .filter_map(|s| match s {
            Step::Run { program, args, .. } => Some(format!("{program} {}", args.join(" "))),
            _ => None,
        })
        .collect();
    let joined = joined.join("\n");
    assert!(
        joined.contains("--bin notes --bin notes-preview"),
        "{joined}"
    );
    assert!(
        joined.contains("notes-preview-universal"),
        "both binaries are merged"
    );
    let appex = joined
        .find("--entitlements /work/target/release/bundle/extension.entitlements /work/target/release/bundle/Notes Pro.app/Contents/PlugIns/Notes Preview.appex")
        .expect("the extension is signed in the sandbox");
    let app = joined
        .find("--sign ABC123 /work/target/release/bundle/Notes Pro.app\n")
        .expect("then the app");
    assert!(appex < app);
    // An extension binary must be a bin target.
    bundle["extensions"][0]["binary"] = json!("missing");
    let m = manifest_from_json(&metadata(bundle), None).unwrap();
    assert!(options_from_manifest(&m, "/b/notes".into(), "/o/N.app".into()).is_err());
}
