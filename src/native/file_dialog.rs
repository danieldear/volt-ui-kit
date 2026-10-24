//! Safe public macOS file panels, opt-in through `native-desktop`.
//! Dialogs are synchronous application-modal UI; do file I/O on a worker afterward.
use std::{fmt, path::PathBuf};
#[derive(Clone, Debug)]
pub struct FileDialogOptions {
    title: String,
    suggested_name: Option<String>,
    directory: Option<PathBuf>,
}
impl FileDialogOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            suggested_name: None,
            directory: None,
        }
    }
    pub fn suggested_name(mut self, name: impl Into<String>) -> Self {
        self.suggested_name = Some(name.into());
        self
    }
    /// Start the panel in this absolute directory instead of the OS's last
    /// remembered location. Existing callers retain their default behavior.
    pub fn directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.directory = Some(directory.into());
        self
    }
    /// Pure configuration validation; never initializes AppKit or opens a panel.
    pub fn validate(&self) -> Result<(), FileDialogError> {
        if self.title.trim().is_empty()
            || self.directory.as_ref().is_some_and(|p| {
                !p.is_absolute()
                    || p.components().any(|c| matches!(c, std::path::Component::ParentDir))
                    || p.as_os_str().as_encoded_bytes().contains(&0)
            })
            || self.title.len() > 256
            || self.title.chars().any(char::is_control)
            || self.suggested_name.as_ref().is_some_and(|name| {
                name.is_empty()
                    || name.len() > 255
                    || name == "."
                    || name == ".."
                    || name
                        .chars()
                        .any(|c| c.is_control() || c == '/' || c == '\\')
            })
        {
            return Err(FileDialogError::InvalidOptions);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileDialogError {
    InvalidOptions,
    Unsupported,
    WrongThread,
    NativeFailure,
}
impl fmt::Display for FileDialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidOptions => "invalid file dialog title, suggested name, or directory",
            Self::Unsupported => "file dialogs require native-desktop on macOS",
            Self::WrongThread => "AppKit file dialogs require the main thread",
            Self::NativeFailure => "native file panel failed or returned an invalid file URL",
        })
    }
}
impl std::error::Error for FileDialogError {}
/// Compiled adapter availability, not an authorization/runtime guarantee.
pub fn available() -> bool {
    cfg!(all(feature = "native-desktop", target_os = "macos"))
}
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(all(feature = "native-desktop", target_os = "macos")),
    allow(dead_code)
)]
enum Panel {
    OpenFile,
    OpenFolder,
    OpenEither,
    Save,
}
pub fn open_file(options: &FileDialogOptions) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, Panel::OpenFile)
}
/// Choose a folder, for example a workspace to browse.
pub fn open_folder(options: &FileDialogOptions) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, Panel::OpenFolder)
}
/// Choose a file or a folder, as a combined Open… command does.
pub fn open_file_or_folder(
    options: &FileDialogOptions,
) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, Panel::OpenEither)
}
/// Ask the native file manager to reveal one existing absolute path, without
/// opening or executing its contents. macOS-only with `native-desktop`.
/// Success means the request was dispatched, not that Finder was observed.
pub fn reveal_file(path: &std::path::Path) -> Result<(), FileDialogError> {
    if !path.is_absolute() {
        return Err(FileDialogError::InvalidOptions);
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        let _main = objc2::MainThreadMarker::new().ok_or(FileDialogError::WrongThread)?;
        std::fs::symlink_metadata(path).map_err(|_| FileDialogError::NativeFailure)?;
        let url =
            objc2_foundation::NSURL::from_file_path(path).ok_or(FileDialogError::InvalidOptions)?;
        let urls = objc2_foundation::NSArray::from_retained_slice(&[url]);
        objc2_app_kit::NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&urls);
        Ok(())
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    Err(FileDialogError::Unsupported)
}
/// Move one existing absolute item to the macOS Trash. Never permanently deletes.
/// Callers own confirmation, protected-path policy and concurrent-change checks.
/// May be called on a worker; no panel or authorization is requested by this API.
pub fn move_to_trash(path: &std::path::Path) -> Result<(), FileDialogError> {
    if !path.is_absolute()
        || path.parent().is_none()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(FileDialogError::InvalidOptions);
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        objc2::rc::autoreleasepool(|_| {
            std::fs::symlink_metadata(path).map_err(|_| FileDialogError::NativeFailure)?;
            let url = objc2_foundation::NSURL::from_file_path(path)
                .ok_or(FileDialogError::InvalidOptions)?;
            objc2_foundation::NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, None)
                .map_err(|_| FileDialogError::NativeFailure)
        })
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    Err(FileDialogError::Unsupported)
}
/// The existing OS-selected Trash on the item's current local filesystem.
/// This is a metadata observation, not authorization or a future restoration guarantee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashCapability {
    pub trash_directory: PathBuf,
}
/// The native operation's resulting location. This is NOT an app Undo token.
/// Finder's Put Back / future restoration availability is not guaranteed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashReceipt {
    pub original_path: PathBuf,
    pub trashed_path: PathBuf,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashError {
    InvalidPath,
    Unsupported,
    Symlink,
    MetadataUnavailable,
    Unavailable,
    CrossVolume,
    NonLocalVolume,
    NativeFailure,
    /// The OS reported success, but a trustworthy resulting location could not
    /// be verified. Record a moved/needs-attention outcome; NEVER retry blindly.
    MovedWithoutVerifiedReceipt,
}
impl TrashError {
    /// If true, retrying the operation could affect a newly created item at the
    /// original path. The caller must record an uncertain moved outcome instead.
    pub fn may_have_moved(self) -> bool {
        self == Self::MovedWithoutVerifiedReceipt
    }
}
impl fmt::Display for TrashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPath => {
                "Trash requires an absolute non-root path without parent traversal"
            }
            Self::Unsupported => "checked Trash requires native-desktop on macOS",
            Self::Symlink => "checked Trash refuses symbolic links in the item or its ancestors",
            Self::MetadataUnavailable => {
                "filesystem metadata is unavailable; item was not submitted to Trash"
            }
            Self::Unavailable => "an existing recoverable Trash could not be verified",
            Self::CrossVolume => "the OS-selected Trash is not on the item's filesystem",
            Self::NonLocalVolume => "checked Trash is unavailable on a non-local volume",
            Self::NativeFailure => "the native Trash operation reported failure",
            Self::MovedWithoutVerifiedReceipt => {
                "the OS moved the item but its Trash receipt could not be verified; do not retry"
            }
        })
    }
}
impl std::error::Error for TrashError {}
fn validate_metadata_path(path: &std::path::Path) -> Result<(), TrashError> {
    if !path.is_absolute()
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(TrashError::InvalidPath);
    }
    Ok(())
}
fn validate_trash_path(path: &std::path::Path) -> Result<(), TrashError> {
    validate_metadata_path(path)?;
    if path.parent().is_none() {
        return Err(TrashError::InvalidPath);
    }
    Ok(())
}
/// Observe whether an existing absolute path is on a local filesystem. This
/// metadata-only query does not need a Trash directory or open file contents.
/// Symbolic links in the path are refused. Unlike Trash operations, volume
/// roots are valid inputs. Requires native-desktop on macOS; elsewhere reports
/// Unsupported. Call on a worker: even metadata syscalls on stale mounts may
/// block, and this is NOT a nonblocking or fd-relative security guarantee.
pub fn is_local_volume(path: &std::path::Path) -> Result<bool, TrashError> {
    validate_metadata_path(path)?;
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        checked_trash::local_volume(path)
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    Err(TrashError::Unsupported)
}
/// Find an existing, symlink-free, same-volume Trash using public Foundation
/// `URLForDirectory` with `create: false`. No directory is created and no
/// trial move/write is performed. Local-volume metadata must be available.
///
/// This does not scan contents, prove free space, obtain permissions, guarantee
/// Put Back, or eliminate concurrent filesystem races. Callers still own
/// confirmation, protected paths and just-in-time content/identity validation.
/// May be called on a worker; no panel or permission prompt is requested.
///
/// ```no_run
/// use volt_ui_kit::native::file_dialog::recoverable_trash_for;
/// # fn check(path: &std::path::Path) {
/// match recoverable_trash_for(path) {
///     Ok(capability) => println!("Existing Trash: {}", capability.trash_directory.display()),
///     Err(problem) => eprintln!("Trash is not available: {problem}"),
/// }
/// # }
/// ```
pub fn recoverable_trash_for(path: &std::path::Path) -> Result<TrashCapability, TrashError> {
    validate_trash_path(path)?;
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        objc2::rc::autoreleasepool(|_| checked_trash::capability(path))
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    Err(TrashError::Unsupported)
}
/// Perform exactly one native Trash request after checking the current local,
/// existing same-volume Trash. No permanent delete or copy/delete fallback is
/// implemented. An earlier capability result is never reused as permission.
///
/// A success receipt verifies the returned item identity at that instant, not
/// future recoverability or Finder Put Back. Path-based OS calls retain a
/// concurrent-change window; this is not a transactional filesystem API.
///
/// ```no_run
/// use volt_ui_kit::native::file_dialog::move_to_trash_checked;
/// # fn confirmed_action(path: &std::path::Path) {
/// // Run only after explicit confirmation and the application's safety checks.
/// match move_to_trash_checked(path) {
///     Ok(receipt) => println!("Moved to {}", receipt.trashed_path.display()),
///     Err(error) if error.may_have_moved() => eprintln!("Needs attention; do not retry: {error}"),
///     Err(error) => eprintln!("Not moved: {error}"),
/// }
/// # }
/// ```
pub fn move_to_trash_checked(path: &std::path::Path) -> Result<TrashReceipt, TrashError> {
    validate_trash_path(path)?;
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        objc2::rc::autoreleasepool(|_| checked_trash::move_item(path))
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    Err(TrashError::Unsupported)
}
#[cfg(all(feature = "native-desktop", target_os = "macos"))]
mod checked_trash {
    use super::*;
    use objc2_foundation::{NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask, NSURL};
    use std::{
        fs::{self, Metadata},
        os::unix::fs::MetadataExt,
        path::Path,
    };

    // Public Darwin SDK sys/mount.h MNT_LOCAL. rustix exposes the platform
    // statfs struct but no MNT_* bitflags on Darwin. Unknown flags fail closed.
    const MNT_LOCAL: u32 = 0x0000_1000;
    pub(super) fn metadata(path: &Path) -> Result<Metadata, TrashError> {
        validate_metadata_path(path)?;
        let mut item = None;
        for ancestor in path.ancestors() {
            let meta =
                fs::symlink_metadata(ancestor).map_err(|_| TrashError::MetadataUnavailable)?;
            if meta.file_type().is_symlink() {
                return Err(TrashError::Symlink);
            }
            if item.is_none() {
                item = Some(meta);
            }
        }
        item.ok_or(TrashError::MetadataUnavailable)
    }
    pub(super) fn verify_local_flags(flags: u32) -> Result<(), TrashError> {
        if flags & MNT_LOCAL == 0 {
            Err(TrashError::NonLocalVolume)
        } else {
            Ok(())
        }
    }
    pub(super) fn local_volume(path: &Path) -> Result<bool, TrashError> {
        metadata(path)?;
        let status = rustix::fs::statfs(path).map_err(|_| TrashError::MetadataUnavailable)?;
        Ok(status.f_flags & MNT_LOCAL != 0)
    }
    fn require_local(path: &Path) -> Result<(), TrashError> {
        let status = rustix::fs::statfs(path).map_err(|_| TrashError::MetadataUnavailable)?;
        verify_local_flags(status.f_flags)
    }
    pub(super) fn verify_volume_ids(source: u64, destination: u64) -> Result<(), TrashError> {
        if source == destination {
            Ok(())
        } else {
            Err(TrashError::CrossVolume)
        }
    }
    pub(super) fn pair(path: &Path, trash: &Path) -> Result<Metadata, TrashError> {
        let source = metadata(path)?;
        let destination = metadata(trash)?;
        if !destination.is_dir() || path.starts_with(trash) || trash.starts_with(path) {
            return Err(TrashError::Unavailable);
        }
        verify_volume_ids(source.dev(), destination.dev())?;
        require_local(path)?;
        require_local(trash)?;
        Ok(source)
    }
    pub(super) fn capability(path: &Path) -> Result<TrashCapability, TrashError> {
        metadata(path)?;
        require_local(path)?;
        let url = NSURL::from_file_path(path).ok_or(TrashError::InvalidPath)?;
        let trash = NSFileManager::defaultManager()
            .URLForDirectory_inDomain_appropriateForURL_create_error(
                NSSearchPathDirectory::TrashDirectory,
                NSSearchPathDomainMask::UserDomainMask,
                Some(&url),
                false,
            )
            .map_err(|_| TrashError::Unavailable)?;
        let trash_directory = trash.to_file_path().ok_or(TrashError::Unavailable)?;
        pair(path, &trash_directory)?;
        Ok(TrashCapability { trash_directory })
    }
    pub(super) fn verify_receipt(
        path: &Path,
        trash: &Path,
        resulting: &Path,
        source: &Metadata,
    ) -> Result<TrashReceipt, TrashError> {
        let verified = || -> Result<(), TrashError> {
            if resulting.parent() != Some(trash) {
                return Err(TrashError::Unavailable);
            }
            let moved = metadata(resulting)?;
            if moved.dev() != source.dev()
                || moved.ino() != source.ino()
                || moved.file_type() != source.file_type()
            {
                return Err(TrashError::Unavailable);
            }
            require_local(resulting)?;
            Ok(())
        };
        verified().map_err(|_| TrashError::MovedWithoutVerifiedReceipt)?;
        Ok(TrashReceipt {
            original_path: path.to_path_buf(),
            trashed_path: resulting.to_path_buf(),
        })
    }
    pub(super) fn move_item(path: &Path) -> Result<TrashReceipt, TrashError> {
        let capability = capability(path)?;
        // Revalidate the metadata gate immediately before the sole native call.
        let source = pair(path, &capability.trash_directory)?;
        let url = NSURL::from_file_path(path).ok_or(TrashError::InvalidPath)?;
        let mut resulting = None;
        NSFileManager::defaultManager()
            .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting))
            .map_err(|_| TrashError::NativeFailure)?;
        let resulting = resulting
            .and_then(|url| url.to_file_path())
            .ok_or(TrashError::MovedWithoutVerifiedReceipt)?;
        verify_receipt(path, &capability.trash_directory, &resulting, &source)
    }
}
pub fn save_file(options: &FileDialogOptions) -> Result<Option<PathBuf>, FileDialogError> {
    options.validate()?;
    show(options, Panel::Save)
}
#[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
fn show(_: &FileDialogOptions, _: Panel) -> Result<Option<PathBuf>, FileDialogError> {
    Err(FileDialogError::Unsupported)
}
#[cfg(all(feature = "native-desktop", target_os = "macos"))]
fn show(options: &FileDialogOptions, kind: Panel) -> Result<Option<PathBuf>, FileDialogError> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSModalResponseCancel, NSModalResponseOK, NSOpenPanel, NSSavePanel};
    use objc2_foundation::NSString;
    let mtm = MainThreadMarker::new().ok_or(FileDialogError::WrongThread)?;
    let open;
    let panel;
    let p: &NSSavePanel = if kind == Panel::Save {
        panel = NSSavePanel::savePanel(mtm);
        &panel
    } else {
        let folders = kind != Panel::OpenFile;
        open = NSOpenPanel::openPanel(mtm);
        open.setCanChooseDirectories(folders);
        open.setCanChooseFiles(kind != Panel::OpenFolder);
        open.setCanCreateDirectories(kind == Panel::OpenFolder);
        open.setAllowsMultipleSelection(false);
        &open
    };
    p.setTitle(Some(&NSString::from_str(&options.title)));
    if let Some(directory) = &options.directory {
        if !directory.is_dir() {
            return Err(FileDialogError::InvalidOptions);
        }
        let url = objc2_foundation::NSURL::from_file_path(directory)
            .ok_or(FileDialogError::InvalidOptions)?;
        p.setDirectoryURL(Some(&url));
    }
    if let Some(name) = &options.suggested_name {
        p.setNameFieldStringValue(&NSString::from_str(name));
    }
    let response = p.runModal();
    if response == NSModalResponseCancel {
        return Ok(None);
    }
    if response != NSModalResponseOK {
        return Err(FileDialogError::NativeFailure);
    }
    let url = p.URL().ok_or(FileDialogError::NativeFailure)?;
    if !url.isFileURL() {
        return Err(FileDialogError::NativeFailure);
    }
    let path = PathBuf::from(
        url.path()
            .ok_or(FileDialogError::NativeFailure)?
            .to_string(),
    );
    if !path.is_absolute() {
        return Err(FileDialogError::NativeFailure);
    }
    Ok(Some(path))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starting_directory_is_opt_in_and_validated_without_opening_a_panel() {
        assert!(FileDialogOptions::new("Save").directory.is_none());
        let absolute = std::env::temp_dir();
        let options = FileDialogOptions::new("Save").directory(&absolute);
        assert_eq!(options.directory.as_ref(), Some(&absolute));
        assert!(options.validate().is_ok());
        for invalid in [PathBuf::from("relative"), absolute.join("../escape")] {
            assert!(FileDialogOptions::new("Save").directory(invalid).validate().is_err());
        }
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    struct Fixture(PathBuf);
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    impl Fixture {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "volt-trash-metadata-{}-{unique}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    #[test]
    fn metadata_gate_rejects_symlink_items_and_ancestors_without_native_calls() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        std::fs::create_dir(&source).unwrap();
        let file = source.join("item");
        std::fs::write(&file, b"generated fixture").unwrap();
        let link = fixture.0.join("link");
        symlink(&source, &link).unwrap();
        assert!(matches!(
            checked_trash::metadata(&link),
            Err(TrashError::Symlink)
        ));
        assert!(matches!(
            checked_trash::metadata(&link.join("item")),
            Err(TrashError::Symlink)
        ));
        assert!(file.exists());
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    #[test]
    fn network_or_unknown_locality_is_refused_without_native_calls() {
        assert_eq!(
            checked_trash::verify_local_flags(0),
            Err(TrashError::NonLocalVolume)
        );
        assert_eq!(
            checked_trash::verify_local_flags(0x8000_0000),
            Err(TrashError::NonLocalVolume)
        );
        assert!(checked_trash::verify_local_flags(0x1000).is_ok());
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    #[test]
    fn different_device_ids_fail_closed_without_native_calls() {
        assert_eq!(
            checked_trash::verify_volume_ids(1, 2),
            Err(TrashError::CrossVolume)
        );
        assert!(checked_trash::verify_volume_ids(3, 3).is_ok());
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    #[test]
    fn metadata_gate_requires_existing_directory_and_disjoint_paths() {
        let fixture = Fixture::new();
        let source = fixture.0.join("item");
        std::fs::write(&source, b"generated fixture").unwrap();
        let trash = fixture.0.join("fake-trash");
        assert!(matches!(
            checked_trash::pair(&source, &trash),
            Err(TrashError::MetadataUnavailable)
        ));
        assert!(!trash.exists(), "metadata preflight created a directory");
        std::fs::create_dir(&trash).unwrap();
        assert_eq!(is_local_volume(&source), Ok(true));
        assert_eq!(is_local_volume(&trash), Ok(true));
        assert!(checked_trash::pair(&source, &trash).is_ok());
        assert!(matches!(
            checked_trash::pair(&fixture.0, &trash),
            Err(TrashError::Unavailable)
        ));
        assert!(matches!(
            checked_trash::pair(&source, &source),
            Err(TrashError::Unavailable)
        ));
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    #[test]
    fn receipt_validation_checks_identity_and_parent_without_invoking_os_trash() {
        let fixture = Fixture::new();
        let original = fixture.0.join("item");
        let trash = fixture.0.join("fake-trash");
        std::fs::create_dir(&trash).unwrap();
        std::fs::write(&original, b"generated fixture").unwrap();
        let metadata = checked_trash::metadata(&original).unwrap();
        let resulting = trash.join("collision-renamed-item");
        // This is a rename entirely inside the generated fixture, NOT native Trash.
        std::fs::rename(&original, &resulting).unwrap();
        let receipt =
            checked_trash::verify_receipt(&original, &trash, &resulting, &metadata).unwrap();
        assert_eq!(receipt.original_path, original);
        assert_eq!(receipt.trashed_path, resulting);
        let unrelated = trash.join("unrelated");
        std::fs::write(&unrelated, b"different object").unwrap();
        assert_eq!(
            checked_trash::verify_receipt(&original, &trash, &unrelated, &metadata),
            Err(TrashError::MovedWithoutVerifiedReceipt)
        );
        assert_eq!(
            checked_trash::verify_receipt(&original, &fixture.0, &resulting, &metadata),
            Err(TrashError::MovedWithoutVerifiedReceipt)
        );
    }

    #[test]
    fn background_thread_never_opens_a_native_panel() {
        let result = std::thread::spawn(|| open_file(&FileDialogOptions::new("Open")))
            .join()
            .unwrap();
        assert_eq!(
            result,
            Err(if available() {
                FileDialogError::WrongThread
            } else {
                FileDialogError::Unsupported
            })
        );
    }
}
