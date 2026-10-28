//! Small AppKit bridge for read-only selected text. No imitation Siri items.
//! The system supplies available lookup, speech and Services actions.
pub fn available() -> bool {
    cfg!(all(target_os = "macos", feature = "native-desktop"))
}
pub fn show(text: &str) -> Result<(), String> {
    if text.is_empty() || text.len() > 64 * 1024 {
        return Err("Select between 1 byte and 64 KiB for macOS text actions".into());
    }
    #[cfg(all(target_os = "macos", feature = "native-desktop"))]
    {
        macos(text)
    }
    #[cfg(not(all(target_os = "macos", feature = "native-desktop")))]
    {
        Err("Native text actions require macOS native-desktop".into())
    }
}
#[cfg(all(target_os = "macos", feature = "native-desktop"))]
fn macos(text: &str) -> Result<(), String> {
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSApplication, NSMenuItem, NSTextView};
    use objc2_foundation::{NSCopying, NSPoint, NSRange, NSRect, NSSize, NSString};
    let mtm = MainThreadMarker::new().ok_or("Native text actions require the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let window = app.keyWindow().ok_or("No active native window")?;
    let view = window.contentView().ok_or("No native content view")?;
    let event = app.currentEvent().ok_or("No native input event")?;
    let text_view = NSTextView::initWithFrame(
        NSTextView::alloc(mtm),
        NSRect::new(NSPoint::new(0., 0.), NSSize::new(0., 0.)),
    );
    text_view.setEditable(false);
    text_view.setSelectable(true);
    text_view.setDrawsBackground(false);
    text_view.setString(&NSString::from_str(text));
    text_view.setSelectedRange(NSRange::new(0, text.encode_utf16().count()));
    // AppKit owns text services and validation. The temporary native text view
    // is the requestor only while the native menu tracks; restore the responder.
    let previous = window.firstResponder();
    view.addSubview(&text_view);
    if !window.makeFirstResponder(Some(&text_view)) {
        text_view.removeFromSuperview();
        return Err("Couldn't make a native text-services requestor".into());
    }
    let Some(menu) = text_view.menuForEvent(&event) else {
        window.makeFirstResponder(previous.as_deref());
        text_view.removeFromSuperview();
        return Err("macOS did not provide text actions for this selection".into());
    };
    // Existing system Services are copied, never removed/reparented from the
    // app menu. Their availability is resolved against NSTextView's selection.
    if let Some(services) = app.servicesMenu() {
        services.update();
        let copy = services.copy();
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str("Services"));
        item.setSubmenu(Some(&copy));
        menu.addItem(&item);
    }
    let point = view.convertPoint_fromView(event.locationInWindow(), None);
    menu.popUpMenuPositioningItem_atLocation_inView(None, point, Some(&view));
    window.makeFirstResponder(previous.as_deref());
    text_view.removeFromSuperview();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_empty_oversized_or_off_thread_without_opening_a_menu() {
        assert!(show("").is_err());
        assert!(show(&"x".repeat(65537)).is_err());
        #[cfg(all(target_os = "macos", feature = "native-desktop"))]
        assert!(std::thread::spawn(|| show("selected"))
            .join()
            .unwrap()
            .is_err());
    }
}
