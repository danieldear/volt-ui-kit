//! Explicit web URL dispatch. Never invoked by composition, painting or hover.
//! Currently macOS-first (`native-desktop`); unsupported platforms fail visibly.
use crate::ui::ExternalUrl;
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenUrlError {
    Unsupported,
    WrongThread,
    NativeFailure,
}
impl fmt::Display for OpenUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unsupported => "opening web URLs requires native-desktop on macOS",
            Self::WrongThread => "opening web URLs requires the main thread",
            Self::NativeFailure => "the operating system could not open the web URL",
        })
    }
}
impl std::error::Error for OpenUrlError {}
/// Compiled adapter availability, not a promise that a browser is installed.
pub fn available() -> bool {
    cfg!(all(feature = "native-desktop", target_os = "macos"))
}
/// Request the default browser through a public OS API. Success means the OS
/// accepted dispatch, not that the site loaded. No shell, subprocess or network
/// access is performed by Volt itself. Call from an explicit user action only.
pub fn open_url(url: &ExternalUrl) -> Result<(), OpenUrlError> {
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    {
        use objc2_app_kit::NSWorkspace;
        use objc2_foundation::{NSString, NSURL};
        let _main = objc2::MainThreadMarker::new().ok_or(OpenUrlError::WrongThread)?;
        let url = NSURL::URLWithString(&NSString::from_str(url.as_str()))
            .ok_or(OpenUrlError::NativeFailure)?;
        if NSWorkspace::sharedWorkspace().openURL(&url) {
            Ok(())
        } else {
            Err(OpenUrlError::NativeFailure)
        }
    }
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    {
        let _ = url;
        Err(OpenUrlError::Unsupported)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_matches_build() {
        assert_eq!(
            available(),
            cfg!(all(feature = "native-desktop", target_os = "macos"))
        );
    }
    #[test]
    #[cfg(not(all(feature = "native-desktop", target_os = "macos")))]
    fn unsupported_dispatch_is_explicit() {
        assert_eq!(
            open_url(&ExternalUrl::parse("https://example.com").unwrap()),
            Err(OpenUrlError::Unsupported)
        );
    }
    #[test]
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    fn background_thread_cannot_launch_a_browser() {
        assert_eq!(
            std::thread::spawn(|| open_url(&ExternalUrl::parse("https://example.com").unwrap()))
                .join()
                .unwrap(),
            Err(OpenUrlError::WrongThread)
        );
    }
}
