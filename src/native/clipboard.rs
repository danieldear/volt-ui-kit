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
