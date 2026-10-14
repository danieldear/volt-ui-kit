//! `cargo volt bundle` from Cargo.toml metadata, and `cargo volt release`:
//! a universal build, Developer ID signing with the hardened runtime,
//! notarization, stapling and a signed disk image. Every external step is a
//! program with arguments (no shell); `--dry-run` prints them.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use super::bundle::{self, AppExtension, BundleOptions, DocumentType, ImportedType};
use super::error;
use serde_json::Value;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

pub const HELP: &str = "cargo volt bundle [--manifest-path Cargo.toml] [--universal]
  Builds the release binary and the .app described by [package.metadata.volt.bundle].
cargo volt release [--manifest-path Cargo.toml] [--universal] [--identity NAME|SHA1]
  [--notary-profile NAME] [--entitlements FILE] [--no-notarize] [--no-dmg] [--dry-run]
  Signs with Developer ID and the hardened runtime, notarizes and staples the
  app, then builds, signs, notarizes and staples a disk image. The identity
  defaults to the only \"Developer ID Application\" certificate in the keychain;
  the notary profile comes from `xcrun notarytool store-credentials NAME`
  (also VOLT_SIGNING_IDENTITY / VOLT_NOTARY_PROFILE or the bundle metadata).

[package.metadata.volt.bundle]
name = \"My App\"                       # default: the package name
identifier = \"com.example.myapp\"      # required
icon = \"assets/AppIcon.icns\"          # relative to Cargo.toml
minimum-system-version = \"13.0\"
category = \"public.app-category.productivity\"
copyright = \"© 2026 Example\"
aliases = [\"myapp\"]                   # Contents/MacOS/myapp links to the executable
strip = false                         # keep symbol names (stripped by default)
[[package.metadata.volt.bundle.document-types]]
name = \"Markdown Document\"
role = \"Editor\"                       # Editor, Viewer, Shell, None
rank = \"Default\"                      # Owner, Default, Alternate, None
content-types = [\"net.daringfireball.markdown\"]   # or: extensions = [\"mdown\"]
[[package.metadata.volt.bundle.extensions]]   # app extensions, e.g. Quick Look
name = \"My App Preview\"
identifier = \"com.example.myapp.preview\"
binary = \"myapp-preview\"                 # another [[bin]] of the package
point = \"com.apple.quicklook.preview\"
principal-class = \"MyAppPreviewProvider\"
attributes = { QLIsDataBasedPreview = true, QLSupportedContentTypes = [\"net.daringfireball.markdown\"] }";

/// The package and its bundle metadata, from `cargo metadata`.
#[derive(Debug)]
pub struct Manifest {
    pub dir: PathBuf,
    pub target_dir: PathBuf,
    pub package: String,
    pub version: String,
    pub bin: String,
    /// Every bin target, for app extensions' binaries.
    pub bins: Vec<String>,
    pub metadata: Value,
}

/// The package `manifest_path` names (or the workspace's only/root package).
pub fn manifest_from_json(json: &Value, manifest_path: Option<&Path>) -> io::Result<Manifest> {
    let packages = json["packages"]
        .as_array()
        .ok_or_else(|| error("cargo metadata has no packages"))?;
    let wanted = manifest_path.and_then(|p| p.canonicalize().ok());
    let package = packages
        .iter()
        .find(|p| {
            wanted.as_ref().is_some_and(|w| {
                p["manifest_path"]
                    .as_str()
                    .and_then(|m| Path::new(m).canonicalize().ok())
                    .as_ref()
                    == Some(w)
            })
        })
        .or_else(|| (packages.len() == 1).then(|| &packages[0]))
        .or_else(|| {
            let root = json["resolve"]["root"].as_str()?;
            packages.iter().find(|p| p["id"].as_str() == Some(root))
        })
        .ok_or_else(|| error("choose a package with --manifest-path"))?;
    let string = |v: &Value, what: &str| -> io::Result<String> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| error(format!("cargo metadata is missing {what}")))
    };
    let manifest = PathBuf::from(string(&package["manifest_path"], "manifest_path")?);
    let metadata = package["metadata"]["volt"]["bundle"].clone();
    if !metadata.is_object() {
        return Err(error(
            "no [package.metadata.volt.bundle] in Cargo.toml (see cargo volt bundle --help)",
        ));
    }
    let bins: Vec<&str> = package["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| {
            t["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|k| k == "bin"))
        })
        .filter_map(|t| t["name"].as_str())
        .collect();
    let name = string(&package["name"], "package name")?;
    let bin = match metadata["binary"].as_str() {
        Some(b) if bins.contains(&b) => b.to_owned(),
        Some(b) => return Err(error(format!("binary {b} is not a bin target"))),
        None if bins.contains(&name.as_str()) => name.clone(),
        None if bins.len() == 1 => bins[0].to_owned(),
        None => return Err(error("set binary = \"...\" in the bundle metadata")),
    };
    Ok(Manifest {
        dir: manifest
            .parent()
            .ok_or_else(|| error("manifest has no directory"))?
            .to_path_buf(),
        target_dir: PathBuf::from(string(&json["target_directory"], "target_directory")?),
        package: name,
        version: string(&package["version"], "version")?,
        bin,
        bins: bins.iter().map(|b| (*b).to_owned()).collect(),
        metadata,
    })
}

fn strings(v: &Value, key: &str) -> io::Result<Vec<String>> {
    match &v[key] {
        Value::Null => Ok(Vec::new()),
        Value::Array(items) => items
            .iter()
            .map(|i| {
                i.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| error(format!("{key} must list strings")))
            })
            .collect(),
        _ => Err(error(format!("{key} must be an array of strings"))),
    }
}

fn optional(v: &Value, key: &str) -> io::Result<Option<String>> {
    match &v[key] {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(error(format!("{key} must be a string"))),
    }
}

/// Bundle options from the metadata, for `binary`, written to `output`.
/// The bin targets of `extensions` in the bundle metadata.
fn extension_bins(m: &Manifest) -> Vec<String> {
    m.metadata["extensions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["binary"].as_str().map(str::to_owned))
        .collect()
}

pub fn options_from_manifest(
    m: &Manifest,
    binary: PathBuf,
    output: PathBuf,
) -> io::Result<BundleOptions> {
    let b = &m.metadata;
    let path = |p: String| m.dir.join(p);
    let document_types = b["document-types"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            Ok(DocumentType {
                name: optional(d, "name")?.ok_or_else(|| error("document type needs a name"))?,
                role: optional(d, "role")?.unwrap_or_else(|| "Viewer".into()),
                rank: optional(d, "rank")?.unwrap_or_else(|| "Default".into()),
                content_types: strings(d, "content-types")?,
                extensions: strings(d, "extensions")?,
                icon: optional(d, "icon")?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let imported_types = b["imported-types"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            Ok(ImportedType {
                identifier: optional(t, "identifier")?
                    .ok_or_else(|| error("imported type needs an identifier"))?,
                description: optional(t, "description")?.unwrap_or_default(),
                conforms_to: strings(t, "conforms-to")?,
                extensions: strings(t, "extensions")?,
                mime_type: optional(t, "mime-type")?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    // Extension binaries sit beside the app's (merged ones carry the same suffix).
    let universal = binary
        .file_name()
        .is_some_and(|n| n.to_string_lossy() == format!("{}-universal", m.bin));
    let extensions = b["extensions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            let bin =
                optional(e, "binary")?.ok_or_else(|| error("app extension needs a binary"))?;
            if !m.bins.contains(&bin) {
                return Err(error(format!(
                    "app extension binary {bin} is not a bin target"
                )));
            }
            let file = if universal {
                format!("{bin}-universal")
            } else {
                bin
            };
            Ok(AppExtension {
                name: optional(e, "name")?.ok_or_else(|| error("app extension needs a name"))?,
                identifier: optional(e, "identifier")?
                    .ok_or_else(|| error("app extension needs an identifier"))?,
                binary: binary.with_file_name(file),
                point: optional(e, "point")?.ok_or_else(|| error("app extension needs a point"))?,
                principal_class: optional(e, "principal-class")?
                    .ok_or_else(|| error("app extension needs a principal-class"))?,
                attributes: match &e["attributes"] {
                    Value::Null => serde_json::Map::new(),
                    Value::Object(map) => map.clone(),
                    _ => return Err(error("app extension attributes must be a table")),
                },
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let usage = match &b["usage"] {
        Value::Null => Vec::new(),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| {
                v.as_str()
                    .map(|v| (k.clone(), v.to_owned()))
                    .ok_or_else(|| error("usage descriptions must be strings"))
            })
            .collect::<io::Result<_>>()?,
        _ => return Err(error("usage must be a table of KEY = \"text\"")),
    };
    Ok(BundleOptions {
        binary,
        output,
        name: optional(b, "name")?.unwrap_or_else(|| m.package.clone()),
        identifier: optional(b, "identifier")?.ok_or_else(|| error("identifier is required"))?,
        version: m.version.clone(),
        minimum: optional(b, "minimum-system-version")?.unwrap_or_else(|| "13.0".into()),
        build: optional(b, "build-number")?.unwrap_or_else(|| "1".into()),
        resources: strings(b, "resources")?.into_iter().map(path).collect(),
        icon: optional(b, "icon")?.map(path),
        usage,
        sign: true,
        strip: match &b["strip"] {
            Value::Null => true,
            Value::Bool(strip) => *strip,
            _ => return Err(error("strip must be true or false")),
        },
        category: optional(b, "category")?,
        copyright: optional(b, "copyright")?,
        document_types,
        imported_types,
        aliases: strings(b, "aliases")?,
        extensions,
    })
}

#[derive(Debug, Default)]
pub struct ReleaseOptions {
    pub manifest_path: Option<PathBuf>,
    pub universal: bool,
    pub identity: Option<String>,
    pub notary_profile: Option<String>,
    pub entitlements: Option<PathBuf>,
    pub notarize: bool,
    pub dmg: bool,
    pub dry_run: bool,
}

impl ReleaseOptions {
    pub fn parse(args: &[String], release: bool) -> io::Result<Self> {
        let mut o = ReleaseOptions {
            notarize: release,
            dmg: release,
            ..Default::default()
        };
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let mut value = || {
                i += 1;
                args.get(i)
                    .cloned()
                    .ok_or_else(|| error(format!("missing value for {flag}")))
            };
            match flag {
                "--manifest-path" => o.manifest_path = Some(value()?.into()),
                "--universal" => o.universal = true,
                "--identity" if release => o.identity = Some(value()?),
                "--notary-profile" if release => o.notary_profile = Some(value()?),
                "--entitlements" if release => o.entitlements = Some(value()?.into()),
                "--no-notarize" if release => o.notarize = false,
                "--no-dmg" if release => o.dmg = false,
                "--dry-run" if release => o.dry_run = true,
                _ => return Err(error(format!("unknown option {flag}"))),
            }
            i += 1;
        }
        Ok(o)
    }
}

/// One step of a build or release.
#[derive(Debug, PartialEq)]
pub enum Step {
    Run {
        what: &'static str,
        program: String,
        args: Vec<String>,
    },
    /// Assemble and check the .app (in process).
    Bundle,
    /// Replace a previous output under the target directory.
    Remove(PathBuf),
    /// Make a directory holding the app and an Applications link for the image.
    Stage { dir: PathBuf, app: PathBuf },
}

fn run(what: &'static str, program: &str, args: &[&str]) -> Step {
    Step::Run {
        what,
        program: program.into(),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
    }
}

/// Where outputs go: `target/release/bundle/`.
pub fn out_dir(m: &Manifest) -> PathBuf {
    m.target_dir.join("release").join("bundle")
}

fn file_stem(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join("-")
}

/// The steps for `bundle` (no identity) or `release`, in order.
pub fn plan(
    m: &Manifest,
    o: &ReleaseOptions,
    name: &str,
    identity: Option<&str>,
    profile: Option<&str>,
) -> Vec<Step> {
    let out = out_dir(m);
    let app = out.join(format!("{name}.app"));
    let manifest = m.dir.join("Cargo.toml");
    let manifest = manifest.to_string_lossy();
    let mut steps = Vec::new();
    let extension_bins = extension_bins(m);
    let build = |target: Option<&str>| {
        let mut args = vec!["build", "--release", "--bin", m.bin.as_str()];
        for bin in &extension_bins {
            args.extend(["--bin", bin.as_str()]);
        }
        args.extend(["--manifest-path", &manifest]);
        if let Some(t) = target {
            args.extend(["--target", t]);
        }
        run("build", "cargo", &args)
    };
    if o.universal {
        steps.push(build(Some("aarch64-apple-darwin")));
        steps.push(build(Some("x86_64-apple-darwin")));
        for bin in std::iter::once(&m.bin).chain(&extension_bins) {
            let arm = m.target_dir.join("aarch64-apple-darwin/release").join(bin);
            let intel = m.target_dir.join("x86_64-apple-darwin/release").join(bin);
            let fat = out.join(format!("{bin}-universal"));
            steps.push(run(
                "merge Apple silicon and Intel binaries",
                "/usr/bin/lipo",
                &[
                    "-create",
                    "-output",
                    &fat.to_string_lossy(),
                    &arm.to_string_lossy(),
                    &intel.to_string_lossy(),
                ],
            ));
        }
    } else {
        steps.push(build(None));
    }
    steps.push(Step::Remove(app.clone()));
    steps.push(Step::Bundle);
    let Some(identity) = identity else {
        return steps;
    };
    let app_s = app.to_string_lossy().into_owned();
    // Nested code is signed first: each app extension in the app sandbox.
    let entitlements_file = out.join("extension.entitlements");
    let entitlements_file = entitlements_file.to_string_lossy();
    for e in m.metadata["extensions"].as_array().into_iter().flatten() {
        let Some(name) = e["name"].as_str() else {
            continue;
        };
        let appex = app.join("Contents/PlugIns").join(format!("{name}.appex"));
        steps.push(run(
            "sign the app extension in the app sandbox",
            "/usr/bin/codesign",
            &[
                "--force",
                "--options",
                "runtime",
                "--timestamp",
                "--sign",
                identity,
                "--entitlements",
                &entitlements_file,
                &appex.to_string_lossy(),
            ],
        ));
    }
    let mut sign = vec![
        "--force",
        "--options",
        "runtime",
        "--timestamp",
        "--sign",
        identity,
    ];
    let entitlements = o
        .entitlements
        .as_ref()
        .map(|e| e.to_string_lossy().into_owned());
    if let Some(e) = &entitlements {
        sign.extend(["--entitlements", e.as_str()]);
    }
    sign.push(&app_s);
    steps.push(run(
        "sign with Developer ID and the hardened runtime",
        "/usr/bin/codesign",
        &sign,
    ));
    steps.push(run(
        "verify the signature",
        "/usr/bin/codesign",
        &["--verify", "--strict", "--deep", "--verbose=2", &app_s],
    ));
    if let Some(profile) = profile.filter(|_| o.notarize) {
        let zip = out.join(format!("{}.zip", file_stem(name)));
        let zip = zip.to_string_lossy();
        steps.push(Step::Remove(PathBuf::from(zip.as_ref())));
        steps.push(run(
            "zip the app for notarization",
            "/usr/bin/ditto",
            &["-c", "-k", "--sequesterRsrc", "--keepParent", &app_s, &zip],
        ));
        steps.push(notarize(&zip, profile));
        steps.push(run(
            "staple the ticket to the app",
            "/usr/bin/xcrun",
            &["stapler", "staple", &app_s],
        ));
        steps.push(run(
            "check Gatekeeper accepts the app",
            "/usr/sbin/spctl",
            &["--assess", "--type", "execute", "--verbose", &app_s],
        ));
        steps.push(Step::Remove(PathBuf::from(zip.as_ref())));
    }
    if o.dmg {
        let stage = out.join("dmg-stage");
        let dmg = out.join(format!("{}-{}.dmg", file_stem(name), m.version));
        let dmg_s = dmg.to_string_lossy().into_owned();
        steps.push(Step::Remove(stage.clone()));
        steps.push(Step::Remove(dmg.clone()));
        steps.push(Step::Stage {
            dir: stage.clone(),
            app: app.clone(),
        });
        steps.push(run(
            "build the disk image",
            "/usr/bin/hdiutil",
            &[
                "create",
                "-volname",
                name,
                "-srcfolder",
                &stage.to_string_lossy(),
                "-fs",
                "HFS+",
                "-format",
                "UDZO",
                "-ov",
                &dmg_s,
            ],
        ));
        steps.push(Step::Remove(stage));
        steps.push(run(
            "sign the disk image",
            "/usr/bin/codesign",
            &["--force", "--timestamp", "--sign", identity, &dmg_s],
        ));
        if let Some(profile) = profile.filter(|_| o.notarize) {
            steps.push(notarize(&dmg_s, profile));
            steps.push(run(
                "staple the ticket to the image",
                "/usr/bin/xcrun",
                &["stapler", "staple", &dmg_s],
            ));
            steps.push(run(
                "check Gatekeeper accepts the image",
                "/usr/sbin/spctl",
                &[
                    "--assess",
                    "--type",
                    "open",
                    "--context",
                    "context:primary-signature",
                    "--verbose",
                    &dmg_s,
                ],
            ));
        }
    }
    steps
}

fn notarize(path: &str, profile: &str) -> Step {
    run(
        "notarize with Apple (waits for the result)",
        "/usr/bin/xcrun",
        &[
            "notarytool",
            "submit",
            path,
            "--keychain-profile",
            profile,
            "--wait",
            "--output-format",
            "json",
        ],
    )
}

/// The only "Developer ID Application" identity in `security find-identity`.
pub fn developer_id(listing: &str) -> io::Result<String> {
    let ids: Vec<&str> = listing
        .lines()
        .filter(|l| l.contains("\"Developer ID Application:"))
        .filter_map(|l| l.split_whitespace().nth(1))
        .collect();
    match ids.as_slice() {
        [one] => Ok((*one).to_owned()),
        [] => Err(error(
            "no Developer ID Application certificate in the keychain (create one at developer.apple.com, or pass --identity)",
        )),
        _ => Err(error("several Developer ID certificates: choose one with --identity")),
    }
}

/// `notarytool submit --output-format json` must report Accepted.
pub fn accepted(output: &str) -> io::Result<()> {
    let json: Value =
        serde_json::from_str(output.trim()).map_err(|_| error("unreadable notarytool output"))?;
    match json["status"].as_str() {
        Some("Accepted") => Ok(()),
        status => Err(error(format!(
            "notarization {}: see `xcrun notarytool log {} --keychain-profile PROFILE`",
            status.unwrap_or("failed"),
            json["id"].as_str().unwrap_or("ID")
        ))),
    }
}

fn read_manifest(path: Option<&Path>) -> io::Result<Manifest> {
    let mut cmd = Command::new("cargo");
    cmd.args(["metadata", "--format-version", "1", "--no-deps"]);
    if let Some(p) = path {
        cmd.arg("--manifest-path").arg(p);
    }
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(error(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    let json: Value =
        serde_json::from_slice(&out.stdout).map_err(|_| error("unreadable cargo metadata"))?;
    manifest_from_json(&json, path)
}

fn execute(
    step: &Step,
    m: &Manifest,
    bundle_options: &mut Option<BundleOptions>,
) -> io::Result<()> {
    match step {
        Step::Run {
            what,
            program,
            args,
        } => {
            eprintln!("  → {what}");
            let output = Command::new(program).args(args).output()?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !output.status.success() {
                return Err(error(format!(
                    "{program} failed:\n{}{}",
                    stdout,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            if args.first().is_some_and(|a| a == "notarytool") {
                accepted(&stdout)?;
            }
            Ok(())
        }
        Step::Remove(path) => {
            // Only ever remove outputs this tool writes.
            if !path.starts_with(out_dir(m)) {
                return Err(error(
                    "refusing to remove a path outside target/release/bundle",
                ));
            }
            match fs::symlink_metadata(path) {
                Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
                Ok(_) => fs::remove_file(path),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            }
        }
        Step::Bundle => {
            let options = bundle_options
                .take()
                .ok_or_else(|| error("bundle options missing"))?;
            eprintln!(
                "  → assemble{} and check the app",
                if options.strip { ", strip" } else { "" }
            );
            bundle::run(options)
        }
        Step::Stage { dir, app } => {
            fs::create_dir_all(dir)?;
            let name = app.file_name().ok_or_else(|| error("app has no name"))?;
            let copy = Command::new("/usr/bin/ditto")
                .arg(app)
                .arg(dir.join(name))
                .status()?;
            if !copy.success() {
                return Err(error("could not stage the app for the disk image"));
            }
            #[cfg(unix)]
            std::os::unix::fs::symlink("/Applications", dir.join("Applications"))?;
            Ok(())
        }
    }
}

/// `cargo volt bundle` and `cargo volt release` from Cargo.toml metadata.
pub fn main(args: &[String], release: bool) -> io::Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(error(
            "cargo volt bundle and release currently support macOS only",
        ));
    }
    let o = ReleaseOptions::parse(args, release)?;
    let m = read_manifest(o.manifest_path.as_deref())?;
    let out = out_dir(&m);
    let binary = if o.universal {
        out.join(format!("{}-universal", m.bin))
    } else {
        m.target_dir.join("release").join(&m.bin)
    };
    let mut options = options_from_manifest(&m, binary, PathBuf::new())?;
    let name = options.name.clone();
    options.output = out.join(format!("{name}.app"));
    // A release is signed with Developer ID instead of ad hoc.
    options.sign = !release;
    let identity = if release {
        Some(
            match o
                .identity
                .clone()
                .or_else(|| std::env::var("VOLT_SIGNING_IDENTITY").ok())
                .or_else(|| optional(&m.metadata, "signing-identity").ok().flatten())
            {
                Some(id) => id,
                None => {
                    let listing = Command::new("/usr/bin/security")
                        .args(["find-identity", "-v", "-p", "codesigning"])
                        .output()?;
                    developer_id(&String::from_utf8_lossy(&listing.stdout))?
                }
            },
        )
    } else {
        None
    };
    let profile = o
        .notary_profile
        .clone()
        .or_else(|| std::env::var("VOLT_NOTARY_PROFILE").ok())
        .or_else(|| optional(&m.metadata, "notary-profile").ok().flatten());
    if release && o.notarize && profile.is_none() {
        return Err(error(
            "notarization needs a keychain profile: run `xcrun notarytool store-credentials NAME --apple-id ID --team-id TEAM` once, then pass --notary-profile NAME (or --no-notarize)",
        ));
    }
    let steps = plan(&m, &o, &name, identity.as_deref(), profile.as_deref());
    if o.dry_run {
        for step in &steps {
            match step {
                Step::Run {
                    what,
                    program,
                    args,
                } => println!("# {what}\n{program} {}", args.join(" ")),
                Step::Bundle => println!(
                    "# assemble{} and check {}",
                    if options.strip { ", strip" } else { "" },
                    options.output.display()
                ),
                Step::Remove(p) => println!("# remove {}", p.display()),
                Step::Stage { dir, app } => println!(
                    "# stage {} with an Applications link in {}",
                    app.display(),
                    dir.display()
                ),
            }
        }
        return Ok(());
    }
    fs::create_dir_all(&out)?;
    let mut options = Some(options);
    for step in &steps {
        execute(step, &m, &mut options)?;
    }
    if release {
        println!("Released to {}", out.display());
    }
    Ok(())
}
