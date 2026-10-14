//! Deliberately narrow local macOS packaging; no shell, recursive assets or publish.
// The parser stays available elsewhere to report an explicit unsupported platform.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]
use super::error;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};
const MAX_BINARY: u64 = 1024 * 1024 * 1024;
const MAX_ALIASES: usize = 8;
const MAX_RESOURCE: u64 = 64 * 1024 * 1024;
const USAGE_KEYS: &[&str] = &[
    "NSCameraUsageDescription",
    "NSMicrophoneUsageDescription",
    "NSLocationUsageDescription",
    "NSContactsUsageDescription",
    "NSCalendarsUsageDescription",
    "NSCalendarsFullAccessUsageDescription",
    "NSCalendarsWriteOnlyAccessUsageDescription",
    "NSRemindersUsageDescription",
    "NSRemindersFullAccessUsageDescription",
    "NSPhotoLibraryUsageDescription",
    "NSPhotoLibraryAddUsageDescription",
    "NSAppleEventsUsageDescription",
    "NSSpeechRecognitionUsageDescription",
];
pub const HELP: &str = "cargo volt bundle --binary PATH --output PATH.app --name NAME --identifier REVERSE.DNS.ID --version X.Y.Z --minimum-system-version X.Y[.Z]\n  [--build-number N[.N[.N]]] [--resource FILE ...] [--icon FILE.icns] [--alias NAME ...] [--strip symbols|none]\n  [--usage NSCameraUsageDescription=TEXT ...] [--sign ad-hoc|none]\nmacOS only; prebuilt Mach-O, system dylibs only, no overwrite; local ad-hoc signing is NOT notarization";

/// A file type the app opens: `CFBundleDocumentTypes` entry. Content types
/// name system or imported UTIs; extensions claim types macOS has no UTI for
/// (Launch Services ignores extensions when content types are present, so
/// give one or the other).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DocumentType {
    pub name: String,
    /// Editor, Viewer, Shell or None.
    pub role: String,
    /// Owner, Default, Alternate or None.
    pub rank: String,
    pub content_types: Vec<String>,
    pub extensions: Vec<String>,
    /// An `.icns` file in the bundle's resources, by file name.
    pub icon: Option<String>,
}

/// A type the app declares for others' files: `UTImportedTypeDeclarations`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedType {
    pub identifier: String,
    pub description: String,
    pub conforms_to: Vec<String>,
    pub extensions: Vec<String>,
    pub mime_type: Option<String>,
}

/// An app extension inside the app (`Contents/PlugIns/NAME.appex`), such as
/// a Quick Look preview. Its executable is another binary of the package and
/// runs in the app sandbox.
#[derive(Clone, Debug, PartialEq)]
pub struct AppExtension {
    pub name: String,
    /// Must extend the app's identifier: `com.example.app.preview`.
    pub identifier: String,
    pub binary: PathBuf,
    /// `NSExtensionPointIdentifier`, e.g. `com.apple.quicklook.preview`.
    pub point: String,
    /// The Objective-C class the system instantiates.
    pub principal_class: String,
    /// `NSExtensionAttributes`.
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

/// Entitlements every app extension is signed with: the app sandbox.
pub const EXTENSION_ENTITLEMENTS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>com.apple.security.app-sandbox</key><true/>\n</dict></plist>\n";

impl AppExtension {
    pub fn bundle_name(&self) -> String {
        format!("{}.appex", self.name)
    }
    pub fn executable_name(&self) -> String {
        sanitized(&self.name)
    }
    fn plist(&self, app: &BundleOptions) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n");
        for (key, value) in [
            ("CFBundleInfoDictionaryVersion", "6.0"),
            ("CFBundlePackageType", "XPC!"),
            ("CFBundleExecutable", &self.executable_name()),
            ("CFBundleName", &self.name),
            ("CFBundleDisplayName", &self.name),
            ("CFBundleIdentifier", &self.identifier),
            ("CFBundleShortVersionString", &app.version),
            ("CFBundleVersion", &app.build),
            ("LSMinimumSystemVersion", &app.minimum),
        ] {
            entry(&mut xml, key, value);
        }
        xml.push_str("<key>NSExtension</key><dict>\n");
        entry(&mut xml, "NSExtensionPointIdentifier", &self.point);
        entry(&mut xml, "NSExtensionPrincipalClass", &self.principal_class);
        xml.push_str("<key>NSExtensionAttributes</key>");
        plist_value(
            &mut xml,
            &serde_json::Value::Object(self.attributes.clone()),
        );
        xml.push_str("\n</dict>\n</dict></plist>\n");
        xml
    }
}

/// A JSON value (from Cargo.toml metadata) as a plist value.
fn plist_value(xml: &mut String, v: &serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::Bool(true) => xml.push_str("<true/>"),
        Value::Bool(false) | Value::Null => xml.push_str("<false/>"),
        Value::Number(n) if n.is_i64() || n.is_u64() => {
            xml.push_str(&format!("<integer>{n}</integer>"))
        }
        Value::Number(n) => xml.push_str(&format!("<real>{n}</real>")),
        Value::String(s) => xml.push_str(&format!("<string>{}</string>", escape(s))),
        Value::Array(items) => {
            xml.push_str("<array>");
            for item in items {
                plist_value(xml, item);
            }
            xml.push_str("</array>");
        }
        Value::Object(map) => {
            xml.push_str("<dict>");
            for (key, value) in map {
                xml.push_str(&format!("<key>{}</key>", escape(key)));
                plist_value(xml, value);
            }
            xml.push_str("</dict>");
        }
    }
}

/// Plist attributes from metadata: no nulls, bounded depth, size and text.
fn plist_attributes_valid(v: &serde_json::Value, depth: usize, budget: &mut usize) -> bool {
    use serde_json::Value;
    if depth > 4 || *budget == 0 {
        return false;
    }
    *budget -= 1;
    match v {
        Value::Null => false,
        Value::Bool(_) | Value::Number(_) => true,
        Value::String(s) => s.len() <= 1024,
        Value::Array(items) => items
            .iter()
            .all(|i| plist_attributes_valid(i, depth + 1, budget)),
        Value::Object(map) => map.iter().all(|(k, v)| {
            !k.is_empty()
                && k.len() <= 128
                && k.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-')
                && plist_attributes_valid(v, depth + 1, budget)
        }),
    }
}

#[derive(Debug)]
pub struct BundleOptions {
    pub binary: PathBuf,
    pub output: PathBuf,
    pub name: String,
    pub identifier: String,
    pub version: String,
    pub minimum: String,
    pub build: String,
    pub resources: Vec<PathBuf>,
    pub icon: Option<PathBuf>,
    pub usage: Vec<(String, String)>,
    pub sign: bool,
    /// Strip the bundled executable's symbol table (the input binary is left
    /// alone). Symbols are about a fifth of a Rust release binary; without
    /// them, panics and crash reports show addresses instead of names.
    pub strip: bool,
    /// `LSApplicationCategoryType`, e.g. `public.app-category.productivity`.
    pub category: Option<String>,
    pub copyright: Option<String>,
    pub document_types: Vec<DocumentType>,
    pub imported_types: Vec<ImportedType>,
    /// Other names for the executable, as links beside it in
    /// `Contents/MacOS`: a command-line entry point (`md`, say) that the
    /// binary recognises by the name it was run as.
    pub aliases: Vec<String>,
    /// App extensions in `Contents/PlugIns`.
    pub extensions: Vec<AppExtension>,
}
impl BundleOptions {
    pub fn parse(args: &[String]) -> io::Result<Self> {
        let mut fields = std::collections::HashMap::new();
        let mut resources = Vec::new();
        let mut usage = Vec::new();
        let mut aliases = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let flag = args[i].as_str();
            let value = args
                .get(i + 1)
                .ok_or_else(|| error(format!("missing value for {flag}")))?;
            match flag {
                "--resource" => {
                    if resources.len() >= 64 {
                        return Err(error("at most 64 resources"));
                    }
                    resources.push(PathBuf::from(value));
                }
                "--alias" => {
                    if aliases.len() >= MAX_ALIASES {
                        return Err(error(format!("at most {MAX_ALIASES} aliases")));
                    }
                    aliases.push(value.clone());
                }
                "--usage" => {
                    let (key, text) = value
                        .split_once('=')
                        .ok_or_else(|| error("--usage requires KEY=TEXT"))?;
                    if usage.len() >= USAGE_KEYS.len() {
                        return Err(error("too many usage descriptions"));
                    }
                    usage.push((key.to_owned(), text.to_owned()));
                }
                "--binary"
                | "--output"
                | "--name"
                | "--identifier"
                | "--version"
                | "--minimum-system-version"
                | "--build-number"
                | "--icon"
                | "--sign"
                | "--strip" => {
                    if fields.insert(flag, value.clone()).is_some() {
                        return Err(error(format!("duplicate {flag}")));
                    }
                }
                _ => return Err(error(format!("unknown bundle option {flag}"))),
            }
            i += 2;
        }
        let mut required = |key| {
            fields
                .remove(key)
                .ok_or_else(|| error(format!("{key} required")))
        };
        let binary = required("--binary")?.into();
        let output = required("--output")?.into();
        let name = required("--name")?;
        let identifier = required("--identifier")?;
        let version = required("--version")?;
        let minimum = required("--minimum-system-version")?;
        let build = fields
            .remove("--build-number")
            .unwrap_or_else(|| "1".to_owned());
        let sign = match fields.remove("--sign").as_deref().unwrap_or("ad-hoc") {
            "ad-hoc" => true,
            "none" => false,
            _ => {
                return Err(error(
                    "--sign must be ad-hoc or none; Developer ID is a separate release step",
                ))
            }
        };
        let strip = match fields.remove("--strip").as_deref().unwrap_or("symbols") {
            "symbols" => true,
            "none" => false,
            _ => return Err(error("--strip must be symbols or none")),
        };
        Ok(Self {
            binary,
            output,
            name,
            identifier,
            version,
            minimum,
            build,
            resources,
            icon: fields.remove("--icon").map(PathBuf::from),
            usage,
            sign,
            strip,
            category: None,
            copyright: None,
            document_types: Vec::new(),
            imported_types: Vec::new(),
            aliases,
            extensions: Vec::new(),
        })
    }
    /// The executable file name, shown by Activity Monitor and crash reports:
    /// the app name reduced to ASCII letters, digits, spaces, `-` and `_`.
    pub fn executable_name(&self) -> String {
        sanitized(&self.name)
    }
    /// Where app extensions are signed from: their sandbox entitlements,
    /// next to the app.
    pub fn extension_entitlements(&self) -> PathBuf {
        self.output.with_file_name("extension.entitlements")
    }
    pub fn validate(&self) -> io::Result<()> {
        text(&self.name, 128)?;
        if self.identifier.len() > 255
            || !self.identifier.contains('.')
            || self
                .identifier
                .split('.')
                .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        {
            return Err(error(
                "identifier must contain nonempty reverse-DNS ASCII letters/digits/- segments",
            ));
        }
        if self.version.split('.').count() != 3 {
            return Err(error("version must be X.Y.Z"));
        }
        numeric_version(&self.version)?;
        let build = numeric_version(&self.build)?;
        if build[0] == 0 || build[0] > 9999 || build[1] > 99 || build[2] > 99 {
            return Err(error(
                "build number requires first component 1-9999, remaining components 0-99",
            ));
        }
        numeric_version(&self.minimum)?;
        if self.minimum.split('.').count() < 2 {
            return Err(error("minimum system version must be X.Y[.Z]"));
        }
        if self.output.extension().and_then(|v| v.to_str()) != Some("app")
            || self.output.file_stem().is_none_or(|v| v.is_empty())
        {
            return Err(error("output must be a named .app directory"));
        }
        let binary = regular(&self.binary, MAX_BINARY)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if binary.permissions().mode() & 0o111 == 0 {
                return Err(error("binary must be executable"));
            }
        }
        #[cfg(not(unix))]
        let _ = binary;
        let mut magic = [0; 4];
        File::open(&self.binary)?.read_exact(&mut magic)?;
        if !matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xce]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
        ) {
            return Err(error(
                "binary must be a prebuilt Mach-O executable, not a script",
            ));
        }
        if self.resources.len() > 64 {
            return Err(error("at most 64 resources"));
        }
        let mut names = HashSet::new();
        for path in self.resources.iter().chain(self.icon.iter()) {
            regular(path, MAX_RESOURCE)?;
            let name = filename(path)?;
            // Case-fold collision check also protects the usual macOS filesystem.
            if !names.insert(name.to_lowercase()) {
                return Err(error("duplicate resource/icon filename"));
            }
        }
        if let Some(icon) = &self.icon {
            if icon.extension().and_then(|v| v.to_str()) != Some("icns") {
                return Err(error("icon must be .icns"));
            }
            let mut header = [0; 8];
            File::open(icon)?.read_exact(&mut header)?;
            if &header[..4] != b"icns"
                || u32::from_be_bytes(header[4..8].try_into().unwrap()) as u64
                    != fs::metadata(icon)?.len()
            {
                return Err(error("invalid ICNS header/length"));
            }
        }
        let mut keys = HashSet::new();
        for (key, value) in &self.usage {
            if !USAGE_KEYS.contains(&key.as_str()) || !keys.insert(key) {
                return Err(error("unsupported/duplicate privacy usage key"));
            }
            text(value, 4096)?;
        }
        if let Some(category) = &self.category {
            if !category.starts_with("public.app-category.") || !uti(category) {
                return Err(error("category must be a public.app-category.* identifier"));
            }
        }
        if let Some(copyright) = &self.copyright {
            text(copyright, 256)?;
        }
        if self.extensions.len() > 8 {
            return Err(error("at most 8 app extensions"));
        }
        let mut seen = HashSet::new();
        for e in &self.extensions {
            text(&e.name, 64)?;
            let mut budget = 256;
            let class_ok = !e.principal_class.is_empty()
                && e.principal_class.len() <= 128
                && e.principal_class
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_');
            if e.name.contains(['/', ':'])
                || e.executable_name().is_empty()
                || !uti(&e.identifier)
                || !e.identifier.starts_with(&format!("{}.", self.identifier))
                || !uti(&e.point)
                || !class_ok
                || !plist_attributes_valid(
                    &serde_json::Value::Object(e.attributes.clone()),
                    0,
                    &mut budget,
                )
                || !seen.insert(e.identifier.clone())
                || !seen.insert(e.name.clone())
            {
                return Err(error(
                    "app extensions need a distinct name, an identifier extending the app's, an extension point, an Objective-C class name and plist attributes",
                ));
            }
        }
        let executable = self.executable_name();
        let mut names = HashSet::new();
        for alias in &self.aliases {
            if self.aliases.len() > MAX_ALIASES
                || !extension(alias)
                || alias.eq_ignore_ascii_case(&executable)
                || !names.insert(alias.to_ascii_lowercase())
            {
                return Err(error(
                    "aliases are distinct plain names (letters, digits, - and _) other than the executable's",
                ));
            }
        }
        if self.document_types.len() > 32 || self.imported_types.len() > 32 {
            return Err(error("at most 32 document and 32 imported types"));
        }
        let icons: Vec<String> = self
            .resources
            .iter()
            .chain(self.icon.iter())
            .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
            .collect();
        for d in &self.document_types {
            text(&d.name, 128)?;
            if !["Editor", "Viewer", "Shell", "None"].contains(&d.role.as_str())
                || !["Owner", "Default", "Alternate", "None"].contains(&d.rank.as_str())
            {
                return Err(error(
                    "document type role must be Editor/Viewer/Shell/None and rank Owner/Default/Alternate/None",
                ));
            }
            if d.content_types.is_empty() == d.extensions.is_empty() {
                return Err(error(
                    "a document type lists content types or extensions (Launch Services ignores extensions beside content types)",
                ));
            }
            if d.content_types.len() > 64
                || d.extensions.len() > 64
                || !d.content_types.iter().all(|t| uti(t))
                || !d.extensions.iter().all(|e| extension(e))
            {
                return Err(error(
                    "document type content types must be UTIs and extensions plain ASCII",
                ));
            }
            if d.icon.as_ref().is_some_and(|i| !icons.contains(i)) {
                return Err(error(
                    "document type icon must be the bundle icon or a resource",
                ));
            }
        }
        for t in &self.imported_types {
            text(&t.description, 256)?;
            if !uti(&t.identifier)
                || t.conforms_to.is_empty()
                || t.conforms_to.len() > 16
                || !t.conforms_to.iter().all(|c| uti(c))
                || t.extensions.is_empty()
                || t.extensions.len() > 64
                || !t.extensions.iter().all(|e| extension(e))
                || t.mime_type.as_ref().is_some_and(|m| {
                    m.len() > 128
                        || !m.contains('/')
                        || m.chars().any(|c| c.is_whitespace() || c.is_control())
                })
            {
                return Err(error(
                    "imported type needs a UTI, conformances, extensions and an optional MIME type",
                ));
            }
        }
        Ok(())
    }
    pub fn plist(&self) -> String {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n");
        for (key, value) in [
            ("CFBundleInfoDictionaryVersion", "6.0"),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleExecutable", &self.executable_name()),
            ("CFBundleName", &self.name),
            ("CFBundleDisplayName", &self.name),
            ("CFBundleIdentifier", &self.identifier),
            ("CFBundleShortVersionString", &self.version),
            ("CFBundleVersion", &self.build),
            ("LSMinimumSystemVersion", &self.minimum),
            ("NSPrincipalClass", "NSApplication"),
        ] {
            entry(&mut xml, key, value);
        }
        if let Some(icon) = &self.icon {
            entry(
                &mut xml,
                "CFBundleIconFile",
                icon.file_name().unwrap().to_str().unwrap(),
            );
        }
        for (key, value) in &self.usage {
            entry(&mut xml, key, value);
        }
        if let Some(category) = &self.category {
            entry(&mut xml, "LSApplicationCategoryType", category);
        }
        if let Some(copyright) = &self.copyright {
            entry(&mut xml, "NSHumanReadableCopyright", copyright);
        }
        if !self.document_types.is_empty() {
            xml.push_str("<key>CFBundleDocumentTypes</key><array>\n");
            for d in &self.document_types {
                xml.push_str("<dict>\n");
                entry(&mut xml, "CFBundleTypeName", &d.name);
                entry(&mut xml, "CFBundleTypeRole", &d.role);
                entry(&mut xml, "LSHandlerRank", &d.rank);
                if let Some(icon) = &d.icon {
                    entry(&mut xml, "CFBundleTypeIconFile", icon);
                }
                array(&mut xml, "LSItemContentTypes", &d.content_types);
                array(&mut xml, "CFBundleTypeExtensions", &d.extensions);
                xml.push_str("</dict>\n");
            }
            xml.push_str("</array>\n");
        }
        if !self.imported_types.is_empty() {
            xml.push_str("<key>UTImportedTypeDeclarations</key><array>\n");
            for t in &self.imported_types {
                xml.push_str("<dict>\n");
                entry(&mut xml, "UTTypeIdentifier", &t.identifier);
                entry(&mut xml, "UTTypeDescription", &t.description);
                array(&mut xml, "UTTypeConformsTo", &t.conforms_to);
                xml.push_str("<key>UTTypeTagSpecification</key><dict>\n");
                array(&mut xml, "public.filename-extension", &t.extensions);
                if let Some(mime) = &t.mime_type {
                    entry(&mut xml, "public.mime-type", mime);
                }
                xml.push_str("</dict>\n</dict>\n");
            }
            xml.push_str("</array>\n");
        }
        xml.push_str("<key>NSHighResolutionCapable</key><true/>\n</dict></plist>\n");
        xml
    }
}
/// A uniform type identifier: dotted ASCII letters, digits, `-` and `_`.
fn uti(value: &str) -> bool {
    value.len() <= 128
        && value.contains('.')
        && value.split('.').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}
fn extension(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 16
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn array(xml: &mut String, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    xml.push_str(&format!("<key>{}</key><array>", escape(key)));
    for v in values {
        xml.push_str(&format!("<string>{}</string>", escape(v)));
    }
    xml.push_str("</array>\n");
}
/// A name reduced to ASCII letters, digits, spaces, `-` and `_`.
fn sanitized(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .collect();
    let name = name.trim();
    if name.is_empty() {
        "VoltApp".into()
    } else {
        name.into()
    }
}
fn text(value: &str, limit: usize) -> io::Result<()> {
    if value.trim().is_empty() || value.len() > limit || value.chars().any(char::is_control) {
        return Err(error(
            "metadata must be nonblank, bounded and free of control characters",
        ));
    }
    Ok(())
}
fn numeric_version(value: &str) -> io::Result<[u32; 3]> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.is_empty() || parts.len() > 3 || value.len() > 32 {
        return Err(error("version must contain 1-3 numeric components"));
    }
    let mut result = [0; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return Err(error("version component must be numeric"));
        }
        result[i] = p
            .parse()
            .map_err(|_| error("version component too large"))?;
    }
    Ok(result)
}
fn entry(xml: &mut String, key: &str, value: &str) {
    xml.push_str(&format!(
        "<key>{}</key><string>{}</string>\n",
        escape(key),
        escape(value)
    ));
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn regular(path: &Path, maximum: u64) -> io::Result<fs::Metadata> {
    let m = fs::symlink_metadata(path)?;
    if !m.is_file() || m.len() > maximum {
        return Err(error(format!(
            "{} must be a bounded regular file (no symlinks/directories)",
            path.display()
        )));
    }
    Ok(m)
}
fn filename(path: &Path) -> io::Result<&str> {
    let name = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| error("resource filename must be UTF-8"))?;
    text(name, 255)?;
    if name.contains(['/', '\\']) {
        return Err(error("invalid resource filename"));
    }
    Ok(name)
}
struct Cleanup {
    path: PathBuf,
    keep: bool,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
pub(super) fn copy(source: &Path, dest: &Path, maximum: u64, executable: bool) -> io::Result<()> {
    regular(source, maximum)?;
    let source = File::open(source)?;
    if !source.metadata()?.is_file() {
        return Err(error("opened source is not a regular file"));
    }
    let mut out = File::options().write(true).create_new(true).open(dest)?;
    if io::copy(&mut source.take(maximum + 1), &mut out)? > maximum {
        return Err(error("source grew beyond copy limit"));
    }
    out.flush()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            dest,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
        )?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}
/// Pure construction, used by tests; production run additionally audits and signs.
pub fn assemble(options: &BundleOptions) -> io::Result<()> {
    options.validate()?;
    // Atomic reservation refuses existing directories, files and dangling symlinks.
    fs::create_dir(&options.output)?;
    let mut cleanup = Cleanup {
        path: options.output.clone(),
        keep: false,
    };
    let contents = options.output.join("Contents");
    fs::create_dir(&contents)?;
    fs::create_dir(contents.join("MacOS"))?;
    fs::create_dir(contents.join("Resources"))?;
    copy(
        &options.binary,
        &contents.join("MacOS").join(options.executable_name()),
        MAX_BINARY,
        true,
    )?;
    // Relative links, so they survive the bundle being moved or copied.
    #[cfg(unix)]
    for alias in &options.aliases {
        std::os::unix::fs::symlink(
            options.executable_name(),
            contents.join("MacOS").join(alias),
        )?;
    }
    #[cfg(not(unix))]
    if let Some(alias) = options.aliases.first() {
        return Err(error(format!(
            "executable aliases need symbolic links: {alias}"
        )));
    }
    for e in &options.extensions {
        let dir = contents
            .join("PlugIns")
            .join(e.bundle_name())
            .join("Contents");
        fs::create_dir_all(dir.join("MacOS"))?;
        copy(
            &e.binary,
            &dir.join("MacOS").join(e.executable_name()),
            MAX_BINARY,
            true,
        )?;
        fs::write(dir.join("Info.plist"), e.plist(options))?;
    }
    for source in options.resources.iter().chain(options.icon.iter()) {
        copy(
            source,
            &contents.join("Resources").join(filename(source)?),
            MAX_RESOURCE,
            false,
        )?;
    }
    fs::write(contents.join("Info.plist"), options.plist())?;
    fs::write(contents.join("PkgInfo"), b"APPL????")?;
    cleanup.keep = true;
    Ok(())
}
pub fn validate_headers(output: &str) -> io::Result<()> {
    let mut count = 0;
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields
            .first()
            .is_some_and(|v| v.starts_with("MH_MAGIC") || v.starts_with("MH_CIGAM"))
        {
            if fields.get(4) != Some(&"EXECUTE") {
                return Err(error("Mach-O slice is not an executable"));
            }
            count += 1;
        }
    }
    if count == 0 {
        return Err(error("missing executable Mach-O header receipt"));
    }
    Ok(())
}
pub fn validate_libraries(output: &str) -> io::Result<()> {
    let mut count = 0;
    for line in output.lines().filter(|line| line.starts_with('\t')) {
        let path = line
            .trim()
            .split(" (compatibility version")
            .next()
            .unwrap_or("");
        if !(path.starts_with("/System/Library/") || path.starts_with("/usr/lib/"))
            || Path::new(path)
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(error(format!("non-system library {path}: framework/dylib embedding is not supported by this slice")));
        }
        count += 1;
    }
    if count == 0 {
        return Err(error("no dependency receipt from otool"));
    }
    Ok(())
}
pub fn validate_deployment(output: &str, minimum: &str) -> io::Result<()> {
    let declared = numeric_version(minimum)?;
    let mut found = 0;
    let mut build = false;
    let mut legacy = false;
    for line in output.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.first() == Some(&"cmd") {
            build = fields.get(1) == Some(&"LC_BUILD_VERSION");
            legacy = fields.get(1) == Some(&"LC_VERSION_MIN_MACOSX");
            if fields
                .get(1)
                .is_some_and(|v| v.starts_with("LC_VERSION_MIN_") && *v != "LC_VERSION_MIN_MACOSX")
            {
                return Err(error(
                    "binary contains a non-macOS legacy deployment record",
                ));
            }
        }
        if build && fields.first() == Some(&"platform") && fields.get(1) != Some(&"1") {
            return Err(error("binary is not built for macOS"));
        }
        if (build && fields.first() == Some(&"minos"))
            || (legacy && fields.first() == Some(&"version"))
        {
            let actual = numeric_version(
                fields
                    .get(1)
                    .ok_or_else(|| error("invalid deployment receipt"))?,
            )?;
            if actual > declared {
                return Err(error("declared minimum is below the binary's compiled deployment target; rebuild with MACOSX_DEPLOYMENT_TARGET or raise --minimum-system-version"));
            }
            found += 1;
        }
    }
    if found == 0 {
        return Err(error("missing macOS deployment target in Mach-O"));
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn tool(program: &str, args: &[&str], path: &Path) -> io::Result<String> {
    let output = Command::new(program)
        .args(args)
        .arg(path.canonicalize()?)
        .output()?;
    if !output.status.success() {
        return Err(error(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|_| error("invalid UTF-8 tool receipt"))
}
pub fn run(options: BundleOptions) -> io::Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(error("cargo volt bundle currently supports macOS only"))
    }
    #[cfg(target_os = "macos")]
    {
        assemble(&options)?;
        let mut cleanup = Cleanup {
            path: options.output.clone(),
            keep: false,
        };
        let executable = options
            .output
            .join("Contents/MacOS")
            .join(options.executable_name());
        let plugins = options.output.join("Contents/PlugIns");
        let extension_executables: Vec<PathBuf> = options
            .extensions
            .iter()
            .map(|e| {
                plugins
                    .join(e.bundle_name())
                    .join("Contents/MacOS")
                    .join(e.executable_name())
            })
            .collect();
        if options.strip {
            // Before any signing: strip keeps the linker's ad-hoc signature valid.
            for path in std::iter::once(&executable).chain(&extension_executables) {
                tool("/usr/bin/strip", &[], path)?;
            }
        }
        for (e, path) in options.extensions.iter().zip(&extension_executables) {
            validate_headers(&tool("/usr/bin/otool", &["-hv"], path)?)?;
            validate_libraries(&tool("/usr/bin/otool", &["-L"], path)?)?;
            tool(
                "/usr/bin/plutil",
                &["-lint"],
                &plugins.join(e.bundle_name()).join("Contents/Info.plist"),
            )?;
        }
        if !options.extensions.is_empty() {
            fs::write(options.extension_entitlements(), EXTENSION_ENTITLEMENTS)?;
        }
        tool(
            "/usr/bin/plutil",
            &["-lint"],
            &options.output.join("Contents/Info.plist"),
        )?;
        validate_headers(&tool("/usr/bin/otool", &["-hv"], &executable)?)?;
        validate_libraries(&tool("/usr/bin/otool", &["-L"], &executable)?)?;
        validate_deployment(
            &tool("/usr/bin/otool", &["-l"], &executable)?,
            &options.minimum,
        )?;
        if options.sign {
            // Nested code first: each extension with its sandbox entitlements.
            let entitlements = options.extension_entitlements();
            let entitlements = entitlements.to_string_lossy();
            for e in &options.extensions {
                tool(
                    "/usr/bin/codesign",
                    &["--force", "--sign", "-", "--entitlements", &entitlements],
                    &plugins.join(e.bundle_name()),
                )?;
            }
            tool(
                "/usr/bin/codesign",
                &["--force", "--sign", "-"],
                &options.output,
            )?;
            tool(
                "/usr/bin/codesign",
                &["--verify", "--strict"],
                &options.output,
            )?;
        }
        cleanup.keep = true;
        println!(
            "Created {} ({})",
            options.output.canonicalize()?.display(),
            if options.sign {
                "local ad-hoc signature verified; NOT notarized"
            } else {
                "no bundle signing requested; NOT notarized"
            }
        );
        Ok(())
    }
}
