//! Toolkit-neutral accessibility snapshot and actions. Hosts map these nodes to
//! a platform API (the native host uses AccessKit); the runtime itself has no
//! assistive-technology dependency.
use super::*;

/// What a node is, for assistive technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessRole {
    /// A scroll view or other grouping that carries meaning.
    Group,
    ScrollView,
    Label,
    Button,
    CheckBox,
    Switch,
    TextInput,
    MultilineTextInput,
    Slider,
}
/// One node of the accessibility tree, in document order. `parent` indexes an
/// earlier node; roots have `None`. Bounds are physical pixels in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    pub id: String,
    pub parent: Option<usize>,
    pub role: AccessRole,
    pub label: String,
    /// Current text of an input (never for passwords), or a slider's value.
    pub value: Option<String>,
    /// Slider value, minimum, maximum and step.
    pub numeric: Option<[f32; 4]>,
    pub checked: Option<bool>,
    pub bounds: [f32; 4],
    pub enabled: bool,
    pub focused: bool,
    pub password: bool,
    pub read_only: bool,
}
/// A value a screen reader asks to set.
#[derive(Clone, Debug, PartialEq)]
pub enum AccessValue {
    Text(String),
    Number(f32),
}

impl Runtime {
    // Treat adapter requests as untrusted commands. Check the current mounted
    // target before touching focus, selection, composition or application state.
    fn access_target(&self, id: &str) -> Option<&LayoutBox> {
        self.scope_allows_id(id).then_some(())?;
        self.boxes
            .iter()
            .find(|b| b.id == id && b.enabled && !b.departing && b.layout_visible)
    }
    /// The current accessibility tree. While a modal overlay is open only its
    /// contents are exposed, matching what pointer and keyboard can reach.
    pub fn accessibility_nodes(&self) -> Vec<AccessNode> {
        let mut out: Vec<AccessNode> = Vec::new();
        // (layout depth, index in `out`) of the nearest kept ancestors.
        let mut ancestors: Vec<(usize, usize)> = Vec::new();
        let focused = self.focused.as_deref();
        for b in &self.boxes {
            while ancestors.last().is_some_and(|(depth, _)| *depth >= b.depth) {
                ancestors.pop();
            }
            if b.departing || !b.layout_visible || !self.scope_allows_id(&b.id) {
                continue;
            }
            let [_, _, w, h] = b.bounds.bounds();
            let visible = {
                let [_, _, cw, ch] = b.clip.bounds();
                w > 0. && h > 0. && cw > 0. && ch > 0.
            };
            let role = if b.editor.is_some() {
                Some(if b.editor.as_ref().is_some_and(|e| e.multiline) {
                    AccessRole::MultilineTextInput
                } else {
                    AccessRole::TextInput
                })
            } else if b.slider.is_some() {
                Some(AccessRole::Slider)
            } else {
                match b.kind {
                    Kind::Button => Some(AccessRole::Button),
                    Kind::Checkbox => Some(AccessRole::CheckBox),
                    Kind::Toggle => Some(AccessRole::Switch),
                    Kind::Text | Kind::Badge if !b.text.trim().is_empty() => {
                        Some(AccessRole::Label)
                    }
                    _ if b.scroll.is_some() => Some(AccessRole::ScrollView),
                    _ => None,
                }
            };
            let Some(role) = role.filter(|_| visible) else {
                continue;
            };
            let editor = b.editor.as_ref();
            let (value, label) = match role {
                AccessRole::TextInput | AccessRole::MultilineTextInput => {
                    let placeholder = editor.map(|e| e.placeholder.clone()).unwrap_or_default();
                    // `editor_text` withholds passwords: no characters or length leak.
                    (self.editor_text(&b.id), placeholder)
                }
                _ => (None, b.text.clone()),
            };
            let numeric = b.slider.as_ref().map(|s| {
                let value = self.slider_values.get(&b.id).copied().unwrap_or(s.value);
                [value, s.min, s.max, s.step]
            });
            let index = out.len();
            out.push(AccessNode {
                id: b.id.clone(),
                parent: ancestors.last().map(|(_, i)| *i),
                role,
                label,
                value: value.or_else(|| numeric.map(|n| format!("{}", n[0]))),
                numeric,
                checked: matches!(role, AccessRole::CheckBox | AccessRole::Switch)
                    .then_some(b.checked),
                bounds: b.bounds.bounds(),
                enabled: b.enabled,
                focused: focused == Some(b.id.as_str()),
                password: editor.is_some_and(|e| e.password),
                read_only: editor.is_some_and(|e| e.read_only),
            });
            ancestors.push((b.depth, index));
        }
        out
    }
    /// Move keyboard focus to a node, as a screen reader's focus action does.
    pub fn accessibility_focus(&mut self, id: &str) -> bool {
        let reachable = self.access_target(id).is_some_and(interactive_box);
        if reachable && self.focused.as_deref() != Some(id) {
            self.cancel_input();
            self.focused = Some(id.to_owned());
            return true;
        }
        false
    }
    /// Press a button, checkbox or switch (screen reader "click").
    pub fn accessibility_activate<C>(
        &mut self,
        id: &str,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if !self
            .access_target(id)
            .is_some_and(|b| interactive(b.kind) && b.editor.is_none() && b.slider.is_none())
        {
            return Ok(InputOutcome::default());
        }
        let focused = self.accessibility_focus(id);
        let activated = self.activate(id, handlers, context)?;
        Ok(InputOutcome {
            changed: focused || activated,
            activated,
        })
    }
    /// Replace an input's text or set a slider's value.
    pub fn accessibility_set_value<C>(
        &mut self,
        id: &str,
        value: AccessValue,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        match value {
            AccessValue::Text(text) => {
                if text.len() > editing::MAX_TEXT {
                    return Ok(InputOutcome::default());
                }
                let Some(multiline) = self
                    .access_target(id)
                    .and_then(|b| b.editor.as_ref())
                    .filter(|e| !e.read_only)
                    .map(|e| e.multiline)
                else {
                    return Ok(InputOutcome::default());
                };
                let sanitized = editing::sanitize(&text, multiline);
                if sanitized.len() > editing::MAX_TEXT || (!text.is_empty() && sanitized.is_empty())
                {
                    return Ok(InputOutcome::default());
                }
                let focused = self.accessibility_focus(id);
                self.editor_select_all();
                let mut result = self.text_input(&text, handlers, context)?;
                result.changed |= focused;
                Ok(result)
            }
            AccessValue::Number(n) => {
                let Some(props) = self
                    .access_target(id)
                    .and_then(|b| b.slider.clone())
                    .filter(|_| n.is_finite())
                else {
                    return Ok(InputOutcome::default());
                };
                let focused = self.accessibility_focus(id);
                let activated =
                    self.emit_slider(id, value_controls::snap(&props, n), handlers, context)?;
                Ok(InputOutcome {
                    changed: focused || activated,
                    activated,
                })
            }
        }
    }
    /// Increase or decrease a slider by one step.
    pub fn accessibility_step<C>(
        &mut self,
        id: &str,
        increase: bool,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if self.access_target(id).is_none_or(|b| b.slider.is_none()) {
            return Ok(InputOutcome::default());
        }
        let focused = self.accessibility_focus(id);
        let key = if increase {
            value_controls::ValueKey::Increase
        } else {
            value_controls::ValueKey::Decrease
        };
        let mut result = self.slider_key(key, handlers, context)?;
        result.changed |= focused;
        Ok(result)
    }
}
