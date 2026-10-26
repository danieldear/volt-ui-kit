//! App-owned typed navigation and explicit, validated web links.
use super::*;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A serializable history checkpoint. Restore validates its invariants first.
/// Page-local model, focus and scroll state remain owned by the application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationSnapshot<T> {
    pub entries: Vec<T>,
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NavigationError;
impl fmt::Display for NavigationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            "navigation needs 1..=1024 entries, a valid current index and a matching capacity",
        )
    }
}
impl std::error::Error for NavigationError {}
/// Bounded browser-style history. Navigating after Back discards the forward
/// branch; overflowing capacity discards the oldest entry. Never performs I/O.
#[derive(Clone, Debug)]
pub struct NavigationHistory<T> {
    entries: Vec<T>,
    index: usize,
    capacity: usize,
}
impl<T: Default> Default for NavigationHistory<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
impl<T> NavigationHistory<T> {
    pub fn new(root: T) -> Self {
        Self {
            entries: vec![root],
            index: 0,
            capacity: 128,
        }
    }
    pub fn with_capacity(root: T, capacity: usize) -> Result<Self, NavigationError> {
        if !(1..=1024).contains(&capacity) {
            return Err(NavigationError);
        }
        Ok(Self {
            entries: vec![root],
            index: 0,
            capacity,
        })
    }
    /// Read-only history entries, without allocating a checkpoint.
    pub fn entries(&self) -> &[T] {
        &self.entries
    }
    /// Zero-based current position within entries.
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn current(&self) -> &T {
        &self.entries[self.index]
    }
    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }
    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }
    pub fn back(&mut self) -> bool {
        if !self.can_go_back() {
            return false;
        }
        self.index -= 1;
        true
    }
    pub fn forward(&mut self) -> bool {
        if !self.can_go_forward() {
            return false;
        }
        self.index += 1;
        true
    }
    pub fn reset(&mut self, root: T) {
        self.entries.clear();
        self.entries.push(root);
        self.index = 0;
    }
    /// Invalid checkpoints leave the current history unchanged. Deserialize
    /// untrusted input with your own byte/depth limits before calling restore.
    pub fn restore(&mut self, snapshot: NavigationSnapshot<T>) -> Result<(), NavigationError> {
        if snapshot.entries.is_empty()
            || snapshot.entries.len() > self.capacity
            || snapshot.index >= snapshot.entries.len()
        {
            return Err(NavigationError);
        }
        self.entries = snapshot.entries;
        self.index = snapshot.index;
        Ok(())
    }
}
impl<T: Clone> NavigationHistory<T> {
    pub fn snapshot(&self) -> NavigationSnapshot<T> {
        NavigationSnapshot {
            entries: self.entries.clone(),
            index: self.index,
        }
    }
}
impl<T: PartialEq> NavigationHistory<T> {
    /// No-op for the current destination, retaining its forward branch.
    pub fn push(&mut self, route: T) -> bool {
        if self.current() == &route {
            return false;
        }
        self.entries.truncate(self.index + 1);
        if self.entries.len() == self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(route);
        self.index = self.entries.len() - 1;
        true
    }
    /// Replace the current entry without changing adjacent Back/Forward entries.
    pub fn replace(&mut self, route: T) -> bool {
        if self.current() == &route {
            return false;
        }
        self.entries[self.index] = route;
        true
    }
}
/// Mount only the current route. Use route-specific keys for independent pages;
/// keep their persistent data in application state, not in this container.
pub fn navigation_stack<C, T>(
    history: &NavigationHistory<T>,
    view: impl FnOnce(&T) -> Element<C>,
) -> Container<C> {
    column([view(history.current())])
}

/// An absolute ASCII HTTP(S) URL. Deliberately excludes credentials, local file
/// access and executable/custom schemes. Encode Unicode paths and domains first.
/// Validation is a dispatch policy, not a guarantee of a trustworthy website.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExternalUrl(String);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UrlError;
impl fmt::Display for UrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("expected an absolute HTTP(S) URL, at most 4096 ASCII bytes, without credentials or control characters")
    }
}
impl std::error::Error for UrlError {}
impl ExternalUrl {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, UrlError> {
        let value = value.as_ref();
        if value.len() > 4096
            || !value.is_ascii()
            || value.bytes().any(|b| {
                !b.is_ascii_graphic()
                    || matches!(
                        b,
                        b'\\' | b'<' | b'>' | b'"' | b'`' | b'{' | b'}' | b'|' | b'^'
                    )
            })
        {
            return Err(UrlError);
        }
        let (scheme, rest) = value.split_once("://").ok_or(UrlError)?;
        if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
            return Err(UrlError);
        }
        let authority = rest.split(['/', '?', '#']).next().ok_or(UrlError)?;
        let (host, port) = if let Some(ip) = authority.strip_prefix('[') {
            let (ip, tail) = ip.split_once(']').ok_or(UrlError)?;
            ip.parse::<std::net::Ipv6Addr>().map_err(|_| UrlError)?;
            (
                ip,
                if tail.is_empty() {
                    None
                } else {
                    Some(tail.strip_prefix(':').ok_or(UrlError)?)
                },
            )
        } else {
            let (host, port) = authority
                .split_once(':')
                .map_or((authority, None), |(h, p)| (h, Some(p)));
            if host.is_empty()
                || host.len() > 253
                || host.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || label.starts_with('-')
                        || label.ends_with('-')
                        || !label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
            {
                return Err(UrlError);
            }
            (host, port)
        };
        if host.is_empty()
            || port.is_some_and(|p| {
                p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) || p.parse::<u16>().is_err()
            })
        {
            return Err(UrlError);
        }
        let bytes = value.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' {
                let pair = bytes.get(i + 1..i + 3).ok_or(UrlError)?;
                let hex = std::str::from_utf8(pair).map_err(|_| UrlError)?;
                let decoded = u8::from_str_radix(hex, 16).map_err(|_| UrlError)?;
                if decoded < 0x20 || decoded == 0x7f {
                    return Err(UrlError);
                }
                i += 3;
            } else {
                i += 1;
            }
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for ExternalUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
impl fmt::Display for ExternalUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
fn link_element<C>(label: impl Into<String>) -> Element<C> {
    button(label)
        .variant(ButtonVariant::Ghost)
        .color(ColorToken::Primary)
        .underline(true)
        .height(32.)
        .font_size(14.)
        .label_align(Align::Start)
        .rounded(4.)
        .0
}
/// Focusable, theme-aware web link. Requires a stable key and an explicit
/// `on_open` handler; building and drawing never opens a browser.
pub struct Link<C>(pub(super) Element<C>, Result<ExternalUrl, UrlError>);
pub fn link<C>(label: impl Into<String>, url: impl AsRef<str>) -> Link<C> {
    let mut element = link_element(label);
    let target = ExternalUrl::parse(url);
    element.node.access_link = target.as_ref().ok().map(|url| url.as_str().into());
    Link(element, target)
}
impl<C> Link<C> {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.0.node.enabled = enabled;
        self
    }
    pub fn on_open(mut self, mut callback: impl FnMut(&mut C, ExternalUrl) + 'static) -> Self {
        if let Ok(target) = &self.1 {
            let target = target.clone();
            self.0.callback = Some(Callback::Click(Box::new(move |app| {
                callback(app, target.clone())
            })));
        }
        self
    }
}
impl<C> From<Link<C>> for Element<C> {
    fn from(mut link: Link<C>) -> Self {
        if link.1.is_err()
            || link.0.auto_id
            || link.0.callback.is_none()
            || link
                .0
                .node
                .label
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
        {
            link.0.validation_error =
                Some("link needs a key, label, HTTP(S) URL and on_open handler".into());
        }
        link.0
    }
}
/// A typed internal destination, separate from external URL dispatch.
pub struct NavigationLink<C, T>(pub(super) Element<C>, T);
pub fn navigation_link<C, T>(label: impl Into<String>, route: T) -> NavigationLink<C, T> {
    let mut element = link_element(label);
    element.node.access_link = Some(String::new());
    NavigationLink(element, route)
}
impl<C, T> NavigationLink<C, T> {
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.0.node.enabled = enabled;
        self
    }
}
impl<C, T: Clone + 'static> NavigationLink<C, T> {
    pub fn on_navigate(mut self, mut callback: impl FnMut(&mut C, T) + 'static) -> Self {
        let route = self.1.clone();
        self.0.callback = Some(Callback::Click(Box::new(move |app| {
            callback(app, route.clone())
        })));
        self
    }
}
impl<C, T> From<NavigationLink<C, T>> for Element<C> {
    fn from(mut link: NavigationLink<C, T>) -> Self {
        if link.0.auto_id
            || link.0.callback.is_none()
            || link
                .0
                .node
                .label
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
        {
            link.0.validation_error =
                Some("navigation_link needs a key, label and on_navigate handler".into());
        }
        link.0
    }
}
