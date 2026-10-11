//! Optional bounded UTF-8 file helpers. Call on a worker, not the UI thread.
//! These are not a filesystem sandbox or a multi-process transaction manager.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
/// Current lightweight text editor limit; oversized files are rejected, never truncated.
pub const MAX_TEXT_FILE_BYTES: usize = 8192;
/// Replacement is always explicit. Matching detects an external edit before commit,
/// but cannot eliminate another process's race between the check and rename.
#[derive(Clone, Copy, Debug)]
pub enum SavePolicy<'a> {
    CreateNew,
    MatchContents(&'a str),
    ApprovedReplacement,
}
fn error(kind: io::ErrorKind, message: &str) -> io::Error {
    io::Error::new(kind, message)
}
fn regular(path: &Path) -> io::Result<fs::Metadata> {
    let m = fs::symlink_metadata(path)?;
    if !m.file_type().is_file() {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "expected a regular file, not a link, directory or device",
        ));
    }
    Ok(m)
}
pub fn read_text_file(path: &Path) -> io::Result<String> {
    regular(path)?;
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "expected a regular file",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TEXT_FILE_BYTES {
        return Err(error(
            io::ErrorKind::InvalidData,
            "text file exceeds the 8 KiB editor limit",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| error(io::ErrorKind::InvalidData, "text file is not valid UTF-8"))
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
/// Write a private sibling, sync its contents, then commit without truncating the
/// original. New-file commits use a hard link so an existing path is never replaced.
/// Existing-file rename replacement is platform dependent and may return an error.
/// Unix permission bits are retained; ACLs/xattrs/hard-link identity are not copied.
/// Parent directories are not fsynced: this is not a power-loss durability guarantee.
pub fn save_text_file(path: &Path, text: &str, policy: SavePolicy<'_>) -> io::Result<()> {
    if text.len() > MAX_TEXT_FILE_BYTES {
        return Err(error(
            io::ErrorKind::InvalidInput,
            "text exceeds the 8 KiB editor limit",
        ));
    }
    if path.file_name().is_none() {
        return Err(error(io::ErrorKind::InvalidInput, "missing file name"));
    }
    let original = match fs::symlink_metadata(path) {
        Ok(_) => Some(regular(path)?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if original
        .as_ref()
        .is_some_and(|m| m.permissions().readonly())
    {
        return Err(error(
            io::ErrorKind::PermissionDenied,
            "target is read-only",
        ));
    }
    match policy {
        SavePolicy::CreateNew if original.is_some() => {
            return Err(error(io::ErrorKind::AlreadyExists, "target already exists"))
        }
        SavePolicy::MatchContents(expected) if read_text_file(path)? != expected => {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "file changed on disk; reopen it or use Save As",
            ))
        }
        _ => {}
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut created = None;
    for _ in 0..16 {
        let temporary = parent.join(format!(
            ".volt-save-{}-{stamp}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => {
                created = Some((Temporary(temporary), file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let (temporary, mut file) = created.ok_or_else(|| {
        error(
            io::ErrorKind::AlreadyExists,
            "cannot allocate a private save file",
        )
    })?;
    file.write_all(text.as_bytes())?;
    if let Some(m) = &original {
        file.set_permissions(m.permissions())?;
    }
    file.sync_all()?;
    drop(file);
    // Recheck after writing. This protects ordinary external edits, not malicious races.
    if let SavePolicy::MatchContents(expected) = policy {
        if read_text_file(path)? != expected {
            return Err(error(
                io::ErrorKind::AlreadyExists,
                "file changed while saving; original retained",
            ));
        }
    }
    if original.is_some() {
        regular(path)?;
        fs::rename(&temporary.0, path)?;
    } else {
        fs::hard_link(&temporary.0, path)?;
    }
    Ok(())
}
