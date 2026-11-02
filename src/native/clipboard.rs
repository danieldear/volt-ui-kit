//! Plain-text system clipboard for application commands (a code block's Copy
//! button, "Copy link"). Text fields already handle Cmd/Ctrl+C themselves.

/// Most text copied in one call.
pub const MAX_TEXT: usize = 16 * 1024 * 1024;

/// Put `text` on the system clipboard. Call from a UI callback.
pub fn set_text(text: &str) -> Result<(), String> {
    if text.len() > MAX_TEXT {
        return Err("clipboard text exceeds 16 MiB".into());
    }
    arboard::Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| e.to_string())
}

/// The clipboard's current plain text, if any.
pub fn text() -> Result<String, String> {
    arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map_err(|e| e.to_string())
}

/// Read explicitly requested PNG clipboard data, bounded before copying it.
/// Call on the UI thread. No conversion, files, network or implicit reads.
pub fn png_image(max_bytes: usize) -> Result<Vec<u8>, String> {
    if max_bytes == 0 || max_bytes > 64 * 1024 * 1024 {
        return Err("Invalid clipboard image limit (1 byte to 64 MiB)".into());
    }
    #[cfg(all(target_os="macos", feature="native-desktop"))]
    {
        use objc2_app_kit::NSPasteboard;
        use objc2_foundation::NSString;
        let clipboard=NSPasteboard::generalPasteboard();
        let data=clipboard.dataForType(&NSString::from_str("public.png"))
            .ok_or("No PNG image on the clipboard; attach an image file instead")?;
        if data.len()>max_bytes {return Err("Clipboard image exceeds the allowed size".into());}
        Ok(data.to_vec())
    }
    #[cfg(not(all(target_os="macos", feature="native-desktop")))]
    { Err("PNG clipboard reads require the macOS desktop host".into()) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn invalid_image_limits_never_read_the_system_clipboard() {
        assert!(super::png_image(0).is_err());
        assert!(super::png_image(64*1024*1024+1).is_err());
    }
}
