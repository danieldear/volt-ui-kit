//! A search field. On iOS the host draws UIKit's own search field over it,
//! with its magnifier, clear button and Search key; elsewhere Volt draws
//! Material 3's search bar.
use super::*;

type Change<C> = Box<dyn FnMut(&mut C, String)>;
type Press<C> = Box<dyn FnMut(&mut C)>;

/// See [`search_field`].
pub struct SearchField<C> {
    key: Option<String>,
    query: String,
    placeholder: String,
    change: Option<Change<C>>,
    submit: Option<Press<C>>,
    autofocus: bool,
    height: f32,
}

/// A search field: filter as people type, search on the keyboard's key.
///
/// ```rust,ignore
/// search_field(&app.query)
///     .key("notes-search")
///     .placeholder("Search notes")
///     .on_change(|a: &mut App, q| a.query = q)
/// ```
pub fn search_field<C>(query: impl Into<String>) -> SearchField<C> {
    SearchField {
        key: None,
        query: query.into(),
        placeholder: "Search".into(),
        change: None,
        submit: None,
        autofocus: false,
        height: 44.,
    }
}
impl<C> SearchField<C> {
    /// Required. The clear button is keyed `{key}-clear`.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    /// What the empty field says. Default "Search"; name what's searched.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    /// Required: the query changed.
    pub fn on_change(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.change = Some(Box::new(f));
        self
    }
    /// The keyboard's Search key.
    pub fn on_submit(mut self, f: impl FnMut(&mut C) + 'static) -> Self {
        self.submit = Some(Box::new(f));
        self
    }
    /// Open the keyboard when it appears (a search that was just opened).
    pub fn autofocus(mut self, value: bool) -> Self {
        self.autofocus = value;
        self
    }
    /// Its height where Volt draws it. Default 44; iOS uses its own.
    pub fn height(mut self, value: f32) -> Self {
        self.height = value;
        self
    }
}

impl<C: 'static> From<SearchField<C>> for Element<C> {
    fn from(s: SearchField<C>) -> Self {
        let Some(key) = s.key.filter(|k| !k.is_empty()) else {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some("search_field requires a key".into());
            return e;
        };
        let Some(change) = s.change else {
            let mut e = Element::new(Kind::Column);
            e.validation_error = Some("search_field requires on_change".into());
            return e;
        };
        let change = std::rc::Rc::new(std::cell::RefCell::new(change));
        let clear = change.clone();
        let typed = change.clone();
        let mut field = text_field(s.query.clone())
            .key(key.clone())
            .accessible_label(s.placeholder.clone())
            .placeholder(s.placeholder)
            .autofocus(s.autofocus)
            // The Search key and the clear button finish a search.
            .hide_done_button()
            .on_change(move |c: &mut C, q| (typed.borrow_mut())(c, q))
            .leading_icon(Icon::Search)
            .height(s.height)
            .font_size(17.)
            .rounded(Rounding::Full)
            .background(ColorToken::Base300)
            .border_width(0.);
        if let Some(mut submit) = s.submit {
            field = field.on_submit(move |c: &mut C| submit(c));
        }
        if !s.query.is_empty() {
            field = field.trailing_action(
                format!("{key}-clear"),
                Icon::Close,
                "Clear",
                move |c: &mut C| (clear.borrow_mut())(c, String::new()),
            );
        }
        let mut element: Element<C> = field.into();
        mark_search(&mut element, &key);
        element
    }
}

/// Flag the field as a search field and its drawn decorations as Volt's
/// own, which a host with a native search field hides.
fn mark_search<C>(e: &mut Element<C>, key: &str) {
    if e.node.id == key {
        if let Some(editor) = e.node.editor.as_mut() {
            editor.search = true;
        }
    } else if e.node.id == format!("{key}-slots") {
        e.node.ext.native_hidden = true;
    }
    for child in &mut e.children {
        mark_search(child, key);
    }
}
