#![cfg(feature = "native")]
use std::path::Path;
use volt_ui_kit::native::file_dialog::{
    is_local_volume, move_to_trash_checked, recoverable_trash_for, TrashError,
};

#[test]
fn checked_trash_rejects_bad_paths_before_any_os_operation() {
    for path in ["relative", "/", "/tmp/../etc", "/tmp/nul\0name"] {
        assert_eq!(
            recoverable_trash_for(Path::new(path)),
            Err(TrashError::InvalidPath)
        );
        assert_eq!(
            move_to_trash_checked(Path::new(path)),
            Err(TrashError::InvalidPath)
        );
    }
}

#[test]
fn locality_rejects_invalid_paths_without_native_actions() {
    for path in ["relative", "/tmp/../etc", "/tmp/nul\0name"] {
        assert_eq!(
            is_local_volume(Path::new(path)),
            Err(TrashError::InvalidPath)
        );
    }
}

#[test]
fn only_unverified_post_success_receipts_may_have_moved() {
    assert!(TrashError::MovedWithoutVerifiedReceipt.may_have_moved());
    for error in [
        TrashError::InvalidPath,
        TrashError::Unsupported,
        TrashError::Symlink,
        TrashError::MetadataUnavailable,
        TrashError::Unavailable,
        TrashError::CrossVolume,
        TrashError::NonLocalVolume,
        TrashError::NativeFailure,
    ] {
        assert!(!error.may_have_moved());
        assert!(!error.to_string().is_empty());
    }
}

#[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
#[test]
fn unsupported_adapter_never_attempts_filesystem_or_native_trash_actions() {
    let path = std::env::current_dir()
        .unwrap()
        .join("definitely-not-a-real-file/unsupported-check");
    assert_eq!(is_local_volume(&path), Err(TrashError::Unsupported));
    assert_eq!(recoverable_trash_for(&path), Err(TrashError::Unsupported));
    assert_eq!(move_to_trash_checked(&path), Err(TrashError::Unsupported));
}
