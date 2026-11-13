//! Conservative freedesktop Trash. Rename-only; no recursion, copy/delete or emptying.
//! Capability probes never create directories. Explicit preparation may do so.
//! New directories are 0700; existing desktop-owned group permissions are retained.
//! Owned, symlink-free, non-world-writable directories are required.
use crate::native::file_dialog::{TrashCapability, TrashError as E, TrashReceipt};
use std::{
    fs::{self, Metadata},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Clone)]
struct Context {
    data: PathBuf,
    uid: u32,
}
impl Context {
    fn current() -> Result<Self, E> {
        let data = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")))
            .ok_or(E::Unavailable)?;
        if !data.is_absolute() {
            return Err(E::InvalidPath);
        }
        Ok(Self {
            data,
            uid: rustix::process::getuid().as_raw(),
        })
    }
}
fn metadata(path: &Path) -> Result<Metadata, E> {
    super::super::file_dialog::validate_metadata_path(path)?;
    let mut result = None;
    for ancestor in path.ancestors() {
        let m = fs::symlink_metadata(ancestor).map_err(|_| E::MetadataUnavailable)?;
        if m.file_type().is_symlink() {
            return Err(E::Symlink);
        }
        if result.is_none() {
            result = Some(m);
        }
    }
    result.ok_or(E::MetadataUnavailable)
}
pub(in crate::native) fn local_volume(path: &Path) -> Result<bool, E> {
    metadata(path)?;
    let stat = rustix::fs::statfs(path).map_err(|_| E::MetadataUnavailable)?;
    Ok(matches!(
        stat.f_type as u64,
        0xef53
            | 0x9123683e
            | 0x58465342
            | 0x01021994
            | 0x2fc12fc1
            | 0xf2f52010
            | 0x794c7630
            | 0x4d44
            | 0x2011bab0
            | 0x5346544e
    ))
}
fn require_local(path: &Path) -> Result<(), E> {
    if local_volume(path)? {
        Ok(())
    } else {
        Err(E::NonLocalVolume)
    }
}
fn ancestor(path: &Path) -> Result<PathBuf, E> {
    for item in path.ancestors() {
        if fs::symlink_metadata(item).is_ok() {
            metadata(item)?;
            return Ok(item.to_path_buf());
        }
    }
    Err(E::Unavailable)
}
fn owned_directory(path: &Path, uid: u32, create: bool) -> Result<(), E> {
    if create && !path.exists() {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|_| E::Unavailable)?;
    }
    let m = metadata(path)?;
    if !m.is_dir() || m.uid() != uid || m.mode() & 0o002 != 0 {
        return Err(E::Unavailable);
    }
    Ok(())
}
fn location(path: &Path, ctx: &Context, create: bool) -> Result<(PathBuf, Option<PathBuf>), E> {
    let source = metadata(path)?;
    require_local(path)?;
    let home_existing = ancestor(&ctx.data)?;
    let (trash, top) = if metadata(&home_existing)?.dev() == source.dev() {
        if create {
            metadata(&home_existing)?;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&ctx.data)
                .map_err(|_| E::Unavailable)?;
        }
        (ctx.data.join("Trash"), None)
    } else {
        let mut top = path.to_path_buf();
        while let Some(parent) = top.parent() {
            if metadata(parent)?.dev() != source.dev() {
                break;
            }
            top = parent.to_path_buf();
        }
        let shared = top.join(".Trash");
        let valid_shared = metadata(&shared).is_ok_and(|m| m.is_dir() && m.mode() & 0o1000 != 0);
        let candidate = if valid_shared {
            shared.join(ctx.uid.to_string())
        } else {
            top.join(format!(".Trash-{}", ctx.uid))
        };
        if valid_shared && owned_directory(&candidate, ctx.uid, create).is_err() {
            (top.join(format!(".Trash-{}", ctx.uid)), Some(top))
        } else {
            (candidate, Some(top))
        }
    };
    if path.starts_with(&trash) || trash.starts_with(path) {
        return Err(E::Unavailable);
    }
    owned_directory(&trash, ctx.uid, create)?;
    for child in ["files", "info"] {
        owned_directory(&trash.join(child), ctx.uid, create)?;
    }
    if metadata(&trash)?.dev() != source.dev() {
        return Err(E::CrossVolume);
    }
    require_local(&trash)?;
    Ok((trash, top))
}
pub(in crate::native) fn capability(path: &Path) -> Result<TrashCapability, E> {
    let (trash_directory, _) = location(path, &Context::current()?, false)?;
    Ok(TrashCapability { trash_directory })
}
pub(in crate::native) fn prepare(path: &Path) -> Result<TrashCapability, E> {
    let (trash_directory, _) = location(path, &Context::current()?, true)?;
    Ok(TrashCapability { trash_directory })
}
fn encoded(path: &Path) -> String {
    let mut out = String::new();
    for b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(b) {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
pub(in crate::native) fn move_item(path: &Path, create: bool) -> Result<TrashReceipt, E> {
    move_with(path, &Context::current()?, create)
}
fn move_with(path: &Path, ctx: &Context, create: bool) -> Result<TrashReceipt, E> {
    super::super::file_dialog::validate_trash_path(path)?;
    let (trash, top) = location(path, ctx, create)?;
    let stored = if let Some(top) = top {
        path.strip_prefix(top)
            .map_err(|_| E::InvalidPath)?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    let name = format!(
        "volt-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| E::NativeFailure)?
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let info = trash.join("info").join(format!("{name}.trashinfo"));
    let destination = trash.join("files").join(name);
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&info)
        .map_err(|_| E::NativeFailure)?;
    let write = (|| {
        write!(
            file,
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encoded(&stored),
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
        )?;
        file.sync_all()
    })();
    if write.is_err() {
        let _ = fs::remove_file(&info);
        return Err(E::NativeFailure);
    }
    // Last metadata/same-volume check immediately before the sole atomic rename.
    let source = match metadata(path).and_then(|m| {
        location(path, ctx, false)?;
        Ok(m)
    }) {
        Ok(m) => m,
        Err(e) => {
            let _ = fs::remove_file(&info);
            return Err(e);
        }
    };
    if rustix::fs::renameat_with(
        rustix::fs::CWD,
        path,
        rustix::fs::CWD,
        &destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .is_err()
    {
        let _ = fs::remove_file(&info);
        return Err(E::NativeFailure);
    }
    let moved = metadata(&destination).map_err(|_| E::MovedWithoutVerifiedReceipt)?;
    if moved.dev() != source.dev()
        || moved.ino() != source.ino()
        || moved.file_type() != source.file_type()
    {
        return Err(E::MovedWithoutVerifiedReceipt);
    }
    // No retry after this point: a rename already happened.
    for directory in [trash.join("info"), trash.join("files")] {
        if fs::File::open(directory)
            .and_then(|f| f.sync_all())
            .is_err()
        {
            return Err(E::MovedWithoutVerifiedReceipt);
        }
    }
    Ok(TrashReceipt {
        original_path: path.to_path_buf(),
        trashed_path: destination,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "volt-trash-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn context(&self) -> Context {
            Context {
                data: self.0.join("data"),
                uid: rustix::process::getuid().as_raw(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn probe_never_creates_and_move_preserves_inode_and_metadata() {
        let f = Fixture::new();
        let path = f.0.join("test #?.txt");
        fs::write(&path, "fixture").unwrap();
        let before = metadata(&path).unwrap();
        assert!(location(&path, &f.context(), false).is_err());
        assert!(!f.context().data.exists());
        let receipt = move_with(&path, &f.context(), true).unwrap();
        assert!(!path.exists());
        assert_eq!(metadata(&receipt.trashed_path).unwrap().ino(), before.ino());
        assert_eq!(
            fs::read_to_string(&receipt.trashed_path).unwrap(),
            "fixture"
        );
        let info = f.context().data.join("Trash/info").join(format!(
            "{}.trashinfo",
            receipt.trashed_path.file_name().unwrap().to_str().unwrap()
        ));
        let text = fs::read_to_string(info).unwrap();
        assert!(text.contains("test%20%23%3F.txt"));
        assert!(text.contains("DeletionDate="));
    }
    #[test]
    fn symlinks_and_protected_trash_are_refused() {
        let f = Fixture::new();
        let file = f.0.join("file");
        fs::write(&file, "keep").unwrap();
        let link = f.0.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(move_with(&link, &f.context(), true), Err(E::Symlink));
        assert_eq!(fs::read_to_string(file).unwrap(), "keep");
    }
    #[test]
    fn large_directory_moves_without_descendant_review_or_copy() {
        let f = Fixture::new();
        let folder = f.0.join("many");
        fs::create_dir(&folder).unwrap();
        for i in 0..4100 {
            fs::write(folder.join(i.to_string()), []).unwrap();
        }
        let receipt = move_with(&folder, &f.context(), true).unwrap();
        assert_eq!(fs::read_dir(receipt.trashed_path).unwrap().count(), 4100);
    }
}

#[cfg(test)]
mod desktop_permissions_tests {
    use super::*;
    #[test]
    fn existing_desktop_group_permissions_are_preserved_but_world_write_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "volt-trash-permissions-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let ctx = Context {
            data: root.join("data"),
            uid: rustix::process::getuid().as_raw(),
        };
        let path = root.join("fixture");
        fs::write(&path, "fixture").unwrap();
        location(&path, &ctx, true).unwrap();
        let trash = ctx.data.join("Trash");
        for dir in [&trash, &trash.join("files"), &trash.join("info")] {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o775)).unwrap();
        }
        let receipt = move_with(&path, &ctx, false).unwrap();
        assert!(receipt.trashed_path.exists());
        assert_eq!(metadata(&trash).unwrap().mode() & 0o777, 0o775);
        fs::write(&path, "keep").unwrap();
        fs::set_permissions(&trash, fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(move_with(&path, &ctx, false), Err(E::Unavailable));
        assert_eq!(fs::read_to_string(path).unwrap(), "keep");
        fs::remove_dir_all(root).unwrap();
    }
}
