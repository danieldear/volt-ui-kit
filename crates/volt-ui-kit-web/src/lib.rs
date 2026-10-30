//! Experimental browser host. UI, layout, state and callbacks stay in Rust.
//! The small JS adapter maps browser events/semantics; it never paints controls.
#![forbid(unsafe_code)]
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use volt_ui_kit::{
    declarative::{
        AccessRole, AccessValue, Bindings, CodeInput, CodeKey, EditKey, Handlers, InputKey,
        InputKeyEvent, InputModifiers, InputOutcome, Runtime, SelectionKey,
    },
    rendering::RasterPainter,
    ui::Element,
    UiTheme, Viewport,
};

#[cfg(target_arch = "wasm32")]
pub mod browser;

/// Strict bounded messages from the DOM adapter. No arbitrary Rust invocation.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum WebEvent {
    Resize {
        width: f32,
        height: f32,
        scale: f32,
    },
    Down {
        x: f32,
        y: f32,
    },
    Move {
        x: f32,
        y: f32,
    },
    Up {
        x: f32,
        y: f32,
    },
    Wheel {
        x: f32,
        y: f32,
        dx: f32,
        dy: f32,
    },
    Context {
        x: f32,
        y: f32,
    },
    Leave {},
    Cancel {},
    Focus {
        id: String,
    },
    Activate {
        id: String,
    },
    Text {
        id: String,
        text: String,
    },
    /// Whole public value plus DOM selection, never used for password/code.
    EditorValue {
        id: String,
        text: String,
        anchor: usize,
        caret: usize,
    },
    Select {
        id: String,
        anchor: usize,
        caret: usize,
    },
    Insert {
        id: String,
        text: String,
        #[serde(default)]
        paste: bool,
    },
    EditKey {
        id: String,
        key: String,
        #[serde(default)]
        shift: bool,
        #[serde(default)]
        control: bool,
        #[serde(default)]
        alt: bool,
        #[serde(default)]
        meta: bool,
    },
    Composition {
        id: String,
        phase: String,
        text: String,
    },
    Preedit {
        id: String,
        text: String,
        anchor: usize,
        caret: usize,
    },
    Number {
        id: String,
        value: f32,
    },
    Arrow {
        key: String,
    },
    Escape {},
    Suspend {
        value: bool,
    },
    Motion {
        reduced: bool,
    },
}
impl WebEvent {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > 65_536 {
            return Err("browser event must be 1–65536 bytes".into());
        }
        let event: Self =
            serde_json::from_slice(bytes).map_err(|_| "invalid browser event".to_owned())?;
        let valid = match &event {
            Self::Resize {
                width,
                height,
                scale,
            } => viewport(*width, *height, *scale).is_ok(),
            Self::Down { x, y }
            | Self::Move { x, y }
            | Self::Up { x, y }
            | Self::Context { x, y } => finite_point(*x, *y),
            Self::Wheel { x, y, dx, dy } => finite_point(*x, *y) && finite_point(*dx, *dy),
            Self::Number { value, .. } => value.is_finite(),
            _ => true,
        };
        if !valid {
            return Err("non-finite or excessive browser geometry".into());
        }
        Ok(event)
    }
}
fn finite_point(x: f32, y: f32) -> bool {
    x.is_finite() && y.is_finite() && x.abs() <= 65_536. && y.abs() <= 65_536.
}
fn viewport(width: f32, height: f32, scale: f32) -> Result<Viewport, String> {
    let v =
        Viewport::new(width * scale, height * scale, scale).ok_or("invalid browser viewport")?;
    if width < 1.
        || height < 1.
        || !(0.25..=4.).contains(&scale)
        || v.width > 8192.
        || v.height > 8192.
        || v.width * v.height > 8_388_608.
    {
        return Err("browser viewport exceeds 8MP / 8192px limits".into());
    }
    Ok(v)
}

/// Host-independent state machine; native tests exercise the same event paths
/// as WASM. Own one per mounted canvas. No window, network or desktop imports.
pub struct WebView<C: 'static> {
    context: C,
    view: fn(&C) -> Element<C>,
    runtime: Runtime,
    handlers: Handlers<'static, C>,
    painter: RasterPainter,
    viewport: Viewport,
    suspended: bool,
    pointer: Option<[f32; 2]>,
}
impl<C: 'static> WebView<C> {
    pub fn new(context: C, view: fn(&C) -> Element<C>) -> Result<Self, String> {
        let (document, handlers) = view(&context)
            .build()
            .map_err(|e| e.to_string())?
            .into_parts();
        let runtime =
            Runtime::new(document, &handlers, &Bindings::default()).map_err(|e| e.to_string())?;
        let viewport = viewport(680., 440., 1.)?;
        let mut host = Self {
            context,
            view,
            runtime,
            handlers,
            painter: RasterPainter::new(viewport),
            viewport,
            suspended: false,
            pointer: None,
        };
        host.runtime.set_reduced_motion(true);
        host.layout()?;
        Ok(host)
    }
    /// Discard only derived drawing resources. Application/input state is retained.
    pub fn release_draw_cache(&mut self) {
        self.painter = RasterPainter::new(self.viewport);
    }
    pub fn context(&self) -> &C {
        &self.context
    }
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }
    fn input_target(&self, id: &str, private: bool) -> bool {
        self.runtime.focused() == Some(id)
            && self.runtime.accessibility_nodes().iter().any(|n| {
                n.id == id
                    && n.enabled
                    && (private || !n.password)
                    && matches!(
                        n.role,
                        AccessRole::TextInput | AccessRole::MultilineTextInput
                    )
            })
    }
    /// Trusted copy/cut event only; passwords never produce clipboard text.
    pub fn clipboard(&mut self, cut: bool) -> Result<Option<String>, String> {
        if self.suspended || self.runtime.editing_password() {
            return Ok(None);
        }
        let (outcome, text) = if self.runtime.code_view_focused() {
            self.runtime
                .code_input(
                    if cut { CodeInput::Cut } else { CodeInput::Copy },
                    &mut self.handlers,
                    &mut self.context,
                )
                .map_err(|e| e.to_string())?
        } else if cut {
            let (text, outcome) = self
                .runtime
                .editor_cut(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            (outcome, text)
        } else {
            (InputOutcome::default(), self.runtime.editor_copy())
        };
        if outcome.activated {
            self.recompose()?;
        } else if outcome.changed {
            self.layout()?;
        }
        Ok(text)
    }
    pub fn update(&mut self, change: impl FnOnce(&mut C)) -> Result<(), String> {
        change(&mut self.context);
        self.recompose()
    }
    fn recompose(&mut self) -> Result<(), String> {
        let (doc, handlers) = (self.view)(&self.context)
            .build()
            .map_err(|e| e.to_string())?
            .into_parts();
        self.runtime
            .replace_document(doc, &handlers, &Bindings::default())
            .map_err(|e| e.to_string())?;
        self.handlers = handlers;
        self.layout()
    }
    fn layout(&mut self) -> Result<(), String> {
        // Layout-owned callbacks (responsive sizes, virtual windows, focus and
        // selection) may require another view. Bound convergence, never spin.
        for _ in 0..8 {
            self.runtime
                .layout(self.viewport, &Bindings::default(), &mut self.painter)
                .map_err(|e| e.to_string())?;
            let mut changed = self
                .runtime
                .dispatch_widths(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            changed |= self
                .runtime
                .dispatch_virtual_windows(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            changed |= self
                .runtime
                .dispatch_focus_regions(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            changed |= self
                .runtime
                .dispatch_editor_selections(&mut self.handlers, &mut self.context)
                .map_err(|e| e.to_string())?;
            if !changed {
                return Ok(());
            }
            let (document, handlers) = (self.view)(&self.context)
                .build()
                .map_err(|e| e.to_string())?
                .into_parts();
            self.runtime
                .replace_document(document, &handlers, &Bindings::default())
                .map_err(|e| e.to_string())?;
            self.handlers = handlers;
        }
        Err("browser view did not settle after eight layout callbacks".into())
    }
    pub fn dispatch(&mut self, event: WebEvent) -> Result<bool, String> {
        if let WebEvent::Suspend { value } = event {
            self.suspended = value;
            self.runtime.cancel_input();
            self.runtime.ime_cancel();
            self.runtime.code_pointer_up();
            self.pointer = None;
            self.runtime.set_tooltips_active(!value);
            return Ok(!value);
        }
        if self.suspended {
            return Ok(false);
        }
        let scale = self.viewport.scale;
        match &event {
            WebEvent::Down { x, y } | WebEvent::Move { x, y } | WebEvent::Up { x, y } => {
                self.pointer = Some([x * scale, y * scale])
            }
            WebEvent::Leave {} | WebEvent::Cancel {} => self.pointer = None,
            _ => {}
        }
        let outcome: Result<InputOutcome, _> = match event {
            WebEvent::Resize {
                width,
                height,
                scale,
            } => {
                self.viewport = viewport(width, height, scale)?;
                self.painter.set_viewport(self.viewport);
                self.runtime.cancel_input();
                self.layout()?;
                return Ok(true);
            }
            WebEvent::Down { x, y } => {
                if let Some(outcome) = self
                    .runtime
                    .code_pointer_down(
                        x * scale,
                        y * scale,
                        1,
                        InputModifiers::default(),
                        &mut self.handlers,
                        &mut self.context,
                    )
                    .map_err(|e| e.to_string())?
                {
                    Ok(outcome)
                } else if self
                    .runtime
                    .editor_pointer_down(x * scale, y * scale, false)
                {
                    Ok(InputOutcome {
                        changed: true,
                        activated: false,
                    })
                } else {
                    self.runtime.pointer_down_outcome(
                        x * scale,
                        y * scale,
                        &mut self.handlers,
                        &mut self.context,
                    )
                }
            }
            WebEvent::Move { x, y } => {
                if let Some(outcome) = self
                    .runtime
                    .code_pointer_drag(x * scale, y * scale, &mut self.handlers, &mut self.context)
                    .map_err(|e| e.to_string())?
                {
                    Ok(outcome)
                } else if self.runtime.editor_pointer_drag(x * scale, y * scale) {
                    Ok(InputOutcome {
                        changed: true,
                        activated: false,
                    })
                } else {
                    self.runtime.pointer_move_outcome(
                        x * scale,
                        y * scale,
                        &mut self.handlers,
                        &mut self.context,
                    )
                }
            }
            WebEvent::Up { x, y } => {
                let code = self.runtime.code_pointer_up();
                let mut outcome = self
                    .runtime
                    .pointer_up_outcome(x * scale, y * scale, &mut self.handlers, &mut self.context)
                    .map_err(|e| e.to_string())?;
                outcome.changed |= code;
                Ok(outcome)
            }
            WebEvent::Wheel { x, y, dx, dy } => self.runtime.scroll_outcome(
                x * scale,
                y * scale,
                [dx, dy],
                &mut self.handlers,
                &mut self.context,
            ),
            WebEvent::Focus { id } => {
                let changed = self.runtime.accessibility_focus(&id);
                self.layout()?;
                return Ok(changed);
            }
            WebEvent::Activate { id } => {
                self.runtime
                    .accessibility_activate(&id, &mut self.handlers, &mut self.context)
            }
            WebEvent::Text { id, text } => {
                // Password editing needs a dedicated secret-safe adapter, not
                // a value roundtrip through the public semantic snapshot.
                if self
                    .runtime
                    .accessibility_nodes()
                    .iter()
                    .any(|n| n.id == id && n.password)
                {
                    return Ok(false);
                }
                self.runtime.accessibility_set_value(
                    &id,
                    AccessValue::Text(text),
                    &mut self.handlers,
                    &mut self.context,
                )
            }
            WebEvent::EditorValue {
                id,
                text,
                anchor,
                caret,
            } => {
                if !self.input_target(&id, false)
                    || !text.is_char_boundary(anchor)
                    || !text.is_char_boundary(caret)
                {
                    return Ok(false);
                }
                let mut outcome = self
                    .runtime
                    .accessibility_set_value(
                        &id,
                        AccessValue::Text(text),
                        &mut self.handlers,
                        &mut self.context,
                    )
                    .map_err(|e| e.to_string())?;
                outcome.changed |= self.runtime.editor_select_range(&id, anchor, caret);
                Ok(outcome)
            }
            WebEvent::Select { id, anchor, caret } => {
                if !self.input_target(&id, false) {
                    return Ok(false);
                }
                Ok(InputOutcome {
                    changed: self.runtime.editor_select_range(&id, anchor, caret),
                    activated: false,
                })
            }
            WebEvent::Insert { id, text, paste } => {
                if !self.input_target(&id, true) {
                    return Ok(false);
                }
                if self.runtime.code_view_focused() {
                    self.runtime
                        .code_input(
                            if paste {
                                CodeInput::Paste(text)
                            } else {
                                CodeInput::Insert(text)
                            },
                            &mut self.handlers,
                            &mut self.context,
                        )
                        .map(|r| r.0)
                } else {
                    self.runtime
                        .text_input(&text, &mut self.handlers, &mut self.context)
                }
            }
            WebEvent::Preedit {
                id,
                text,
                anchor,
                caret,
            } => {
                if !self.input_target(&id, true)
                    || !text.is_char_boundary(anchor)
                    || !text.is_char_boundary(caret)
                {
                    return Ok(false);
                }
                Ok(InputOutcome {
                    changed: self.runtime.ime_preedit(&text, Some((anchor, caret))),
                    activated: false,
                })
            }
            WebEvent::Composition { id, phase, text } => {
                if !self.input_target(&id, true) {
                    return Ok(false);
                }
                match phase.as_str() {
                    "update" => Ok(InputOutcome {
                        changed: self
                            .runtime
                            .ime_preedit(&text, Some((text.len(), text.len()))),
                        activated: false,
                    }),
                    "commit" => {
                        self.runtime
                            .ime_commit(&text, &mut self.handlers, &mut self.context)
                    }
                    "cancel" => Ok(InputOutcome {
                        changed: self.runtime.ime_cancel(),
                        activated: false,
                    }),
                    _ => return Err("unknown composition phase".into()),
                }
            }
            WebEvent::EditKey {
                id,
                key,
                shift,
                control,
                alt,
                meta,
            } => {
                if !self.input_target(&id, true) {
                    return Ok(false);
                }
                let modifiers = InputModifiers {
                    shift,
                    control,
                    alt,
                    meta,
                };
                if self.runtime.code_view_focused() {
                    let key = match key.as_str() {
                        "ArrowLeft" => CodeKey::Left,
                        "ArrowRight" => CodeKey::Right,
                        "ArrowUp" => CodeKey::Up,
                        "ArrowDown" => CodeKey::Down,
                        "Home" => CodeKey::Home,
                        "End" => CodeKey::End,
                        "PageUp" => CodeKey::PageUp,
                        "PageDown" => CodeKey::PageDown,
                        "Backspace" => CodeKey::Backspace,
                        "Delete" => CodeKey::Delete,
                        "Enter" => CodeKey::Enter,
                        "Tab" => CodeKey::Tab,
                        "Undo" => CodeKey::Undo,
                        "Redo" => CodeKey::Redo,
                        "SelectAll" => CodeKey::SelectAll,
                        "Escape" => CodeKey::Escape,
                        _ => return Ok(false),
                    };
                    self.runtime
                        .code_input(
                            CodeInput::Key { key, modifiers },
                            &mut self.handlers,
                            &mut self.context,
                        )
                        .map(|r| r.0)
                } else if key == "SelectAll" {
                    Ok(InputOutcome {
                        changed: self.runtime.editor_select_all(),
                        activated: false,
                    })
                } else {
                    let key = match key.as_str() {
                        "ArrowLeft" => EditKey::Left,
                        "ArrowRight" => EditKey::Right,
                        "ArrowUp" => EditKey::Up,
                        "ArrowDown" => EditKey::Down,
                        "Home" => EditKey::Home,
                        "End" => EditKey::End,
                        "Backspace" => EditKey::Backspace,
                        "Delete" => EditKey::Delete,
                        "Enter" => EditKey::Enter,
                        "Undo" => EditKey::Undo,
                        "Redo" => EditKey::Redo,
                        _ => return Ok(false),
                    };
                    self.runtime
                        .editor_key(key, shift, &mut self.handlers, &mut self.context)
                }
            }
            WebEvent::Number { id, value } => self.runtime.accessibility_set_value(
                &id,
                AccessValue::Number(value),
                &mut self.handlers,
                &mut self.context,
            ),
            WebEvent::Arrow { key } => {
                let input_key = match key.as_str() {
                    "ArrowLeft" => InputKey::ArrowLeft,
                    "ArrowRight" => InputKey::ArrowRight,
                    "ArrowUp" => InputKey::ArrowUp,
                    "ArrowDown" => InputKey::ArrowDown,
                    "Home" => InputKey::Home,
                    "End" => InputKey::End,
                    _ => return Ok(false),
                };
                let key = match key.as_str() {
                    "ArrowLeft" | "ArrowUp" => SelectionKey::Previous,
                    "ArrowRight" | "ArrowDown" => SelectionKey::Next,
                    "Home" => SelectionKey::First,
                    "End" => SelectionKey::Last,
                    _ => return Ok(false),
                };
                if self.runtime.overlay_key(key) {
                    Ok(InputOutcome {
                        changed: true,
                        activated: false,
                    })
                } else {
                    let result = self
                        .runtime
                        .input_key(
                            InputKeyEvent {
                                key: input_key,
                                modifiers: InputModifiers::default(),
                                repeat: false,
                            },
                            &mut self.handlers,
                            &mut self.context,
                        )
                        .map_err(|e| e.to_string())?;
                    if result.changed || result.activated {
                        Ok(result)
                    } else {
                        self.runtime
                            .selection_key(key, &mut self.handlers, &mut self.context)
                    }
                }
            }
            WebEvent::Escape {} => self
                .runtime
                .dismiss_overlay(&mut self.handlers, &mut self.context),
            WebEvent::Cancel {} => return Ok(self.runtime.cancel_input()),
            WebEvent::Context { x, y } => self.runtime.context_menu_at(
                x * scale,
                y * scale,
                &mut self.handlers,
                &mut self.context,
            ),
            WebEvent::Leave {} => Ok(InputOutcome {
                changed: self.runtime.pointer_leave(),
                activated: false,
            }),
            WebEvent::Motion { reduced } => {
                self.runtime.set_reduced_motion(reduced);
                return Ok(true);
            }
            WebEvent::Suspend { .. } => unreachable!(),
        };
        let mut outcome = outcome.map_err(|e| e.to_string())?;
        outcome.activated |= self
            .runtime
            .dispatch_hover_regions(self.pointer, &mut self.handlers, &mut self.context)
            .map_err(|e| e.to_string())?;
        if outcome.activated {
            self.recompose()?;
        } else if outcome.changed {
            self.layout()?;
        }
        Ok(outcome.changed || outcome.activated)
    }
    /// Advance only from a browser animation frame while visible. No idle loop.
    pub fn advance(&mut self, milliseconds: f64) -> Result<bool, String> {
        if !milliseconds.is_finite() || !(0. ..=1e12).contains(&milliseconds) {
            return Err("invalid animation timestamp".into());
        }
        if self.suspended {
            return Ok(false);
        }
        let changed = self
            .runtime
            .advance_animations(Duration::from_secs_f64(milliseconds / 1000.));
        if changed {
            self.layout()?;
        }
        Ok(changed)
    }
    pub fn snapshot(&self) -> Value {
        let scale = self.viewport.scale;
        let boxes: std::collections::BTreeMap<_, _> = self
            .runtime
            .boxes()
            .iter()
            .map(|b| (b.id.as_str(), b))
            .collect();
        let nodes:Vec<_>=self.runtime.accessibility_nodes().into_iter().map(|n| {
            let clip=boxes.get(n.id.as_str()).map(|b|b.clip.bounds().map(|v|v/scale)).unwrap_or([0.,0.,0.,0.]);
            let code=self.runtime.code_view_read_only_for(&n.id);
            let selection=if n.password {None}else{self.runtime.editor_selection(&n.id)};
            let role=match n.role {
                AccessRole::TextInput=>"textbox",AccessRole::MultilineTextInput=>"textbox",AccessRole::Label=>"text",AccessRole::ScrollView=>"region",
                AccessRole::Button=>"button",AccessRole::CheckBox=>"checkbox",AccessRole::Switch=>"switch",AccessRole::Slider=>"slider",AccessRole::Tab=>"tab",AccessRole::TabList=>"tablist",
                AccessRole::RadioGroup=>"radiogroup",AccessRole::RadioButton=>"radio",AccessRole::ListBox=>"listbox",AccessRole::ListBoxOption=>"option",AccessRole::Table=>"table",AccessRole::Row=>"row",AccessRole::ColumnHeader=>"columnheader",AccessRole::Cell=>"cell",AccessRole::Tree=>"tree",AccessRole::TreeItem=>"treeitem",AccessRole::Group=>"group",
            };
            json!({"id":n.id,"parent":n.parent,"role":role,"label":n.label,"value":if n.password {None}else{n.value},"placeholder":n.placeholder,"checked":n.checked,"selected":n.selected,"expanded":n.expanded,"numeric":n.numeric,"scroll":n.scroll,"bounds":n.bounds.map(|v|v/scale),"clip":clip,"enabled":n.enabled,"focused":n.focused,"readOnly":n.read_only || code.unwrap_or(false),"code":code.is_some(),"selection":selection,"password":n.password,"clickable":n.clickable,"multiline":n.role==AccessRole::MultilineTextInput})
        }).collect();
        json!({"width":self.viewport.width/scale,"height":self.viewport.height/scale,"scale":scale,"suspended":self.suspended,"animating":!self.suspended && self.runtime.is_animating(),"caret":self.runtime.editor_candidate_bounds().map(|r|r.map(|v|v/scale)),"nodes":nodes})
    }
    /// Software drawing is explicit; Canvas is only a pixel presentation target.
    pub fn rgba(&mut self, theme: &UiTheme) -> Result<(Vec<u8>, u32, u32), String> {
        self.layout()?;
        let batches = self.runtime.paint_ui(&mut self.painter, theme);
        let pixels = self.painter.rasterize(&batches, theme.palette.base[0])?;
        let mut data = pixels.data().to_vec();
        // tiny-skia is premultiplied; ImageData requires straight RGBA.
        for pixel in data.chunks_exact_mut(4) {
            let a = u16::from(pixel[3]);
            if a > 0 && a < 255 {
                for c in &mut pixel[..3] {
                    *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }
        Ok((data, pixels.width(), pixels.height()))
    }
    #[cfg(feature = "gpu")]
    pub fn present(
        &mut self,
        renderer: &mut volt_ui_kit::rendering::SurfaceRenderer,
        theme: &UiTheme,
    ) -> Result<volt_ui_kit::rendering::PresentStatus, String> {
        self.layout()?;
        let batches = self.runtime.paint_ui(&mut self.painter, theme);
        renderer.present(&batches, &mut self.painter, theme.palette.base[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::ui::prelude::*;
    #[derive(Default)]
    struct App {
        count: usize,
        text: String,
        checked: bool,
        tab: u8,
        password: String,
        readonly: bool,
        disabled: bool,
    }
    fn view(a: &App) -> Element<App> {
        vstack![
            button(format!("Count {}", a.count))
                .key("add")
                .on_click(|a: &mut App| a.count += 1),
            button("Disabled")
                .key("disabled")
                .enabled(false)
                .on_click(|a: &mut App| a.count += 100),
            text_field(&a.text)
                .key("name")
                .accessible_label("Name")
                .on_change(|a: &mut App, s| a.text = s),
            text_field(&a.password)
                .key("password")
                .password(true)
                .read_only(a.readonly)
                .enabled(!a.disabled)
                .on_change(|a: &mut App, s| a.password = s),
            checkbox("Check", a.checked)
                .key("check")
                .on_change(|a: &mut App, v| a.checked = v),
            tab_bar([tab_item(0u8, "One"), tab_item(1, "Two")])
                .key("tabs")
                .selected(a.tab)
                .on_select(|a: &mut App, t| a.tab = t),
            vstack((0..30).map(|i| text(format!("Row {i}")).height(32.).into()))
                .key("list")
                .height(100.)
                .scroll(Scroll::Vertical),
        ]
        .padding(12., Edge::All)
        .gap(8., Axis::Vertical)
        .fill_width()
        .into()
    }
    fn host() -> WebView<App> {
        WebView::new(
            App {
                password: "stored-secret-🌿".into(),
                ..App::default()
            },
            view,
        )
        .unwrap()
    }
    #[test]
    fn wire_format_rejects_unknown_fields_invalid_geometry_and_large_payloads() {
        for input in [
            r#"{}"#,
            r#"{"op":"activate"}"#,
            r#"{"op":"cancel","extra":true}"#,
            r#"{"op":"resize","width":10000,"height":10000,"scale":4}"#,
            r#"{"op":"resize","width":0,"height":100,"scale":1}"#,
            r#"{"op":"wheel","x":2,"y":2,"dx":1e99,"dy":0}"#,
        ] {
            assert!(WebEvent::decode(input.as_bytes()).is_err(), "{input}");
        }
        assert!(WebEvent::decode(&vec![b' '; 65537]).is_err());
        assert!(
            WebEvent::decode(r#"{"op":"text","id":"name","text":"Zoë 東京"}"#.as_bytes()).is_ok()
        );
    }
    #[test]
    fn callbacks_recompose_and_unmounted_or_disabled_targets_do_not_activate() {
        let mut h = host();
        assert!(h.dispatch(WebEvent::Activate { id: "add".into() }).unwrap());
        assert_eq!(h.context().count, 1);
        for id in ["disabled", "missing"] {
            assert!(!h.dispatch(WebEvent::Activate { id: id.into() }).unwrap());
        }
        assert_eq!(h.context().count, 1);
        h.dispatch(WebEvent::Text {
            id: "name".into(),
            text: "Zoë 東京".into(),
        })
        .unwrap();
        assert_eq!(h.context().text, "Zoë 東京");
        h.dispatch(WebEvent::Activate { id: "check".into() })
            .unwrap();
        assert!(h.context().checked);
    }
    #[test]
    fn password_values_are_withheld_and_not_roundtripped_through_dom() {
        let mut h = host();
        let snapshot = h.snapshot();
        let password = snapshot["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == "password")
            .unwrap();
        assert!(password["value"].is_null());
        assert_eq!(password["readOnly"], false);
        assert!(password["selection"].is_null());
        assert!(!snapshot.to_string().contains("stored-secret"));
        assert!(!h
            .dispatch(WebEvent::Text {
                id: "password".into(),
                text: "no".into()
            })
            .unwrap());
        assert_eq!(h.context().text, "");
    }
    #[test]
    fn private_password_edits_are_write_only_and_copy_cut_never_export_them() {
        let mut h = host();
        h.dispatch(WebEvent::Focus {
            id: "password".into(),
        })
        .unwrap();
        h.runtime.editor_select_all();
        h.dispatch(WebEvent::Insert {
            id: "password".into(),
            text: "new-private-🔐".into(),
            paste: false,
        })
        .unwrap();
        assert_eq!(h.context().password, "new-private-🔐");
        assert!(!h.snapshot().to_string().contains("new-private"));
        assert!(h.clipboard(false).unwrap().is_none());
        assert!(h.clipboard(true).unwrap().is_none());
        assert_eq!(h.context().password, "new-private-🔐");
        for (readonly, disabled) in [(true, false), (false, true)] {
            h.update(|a| {
                a.readonly = readonly;
                a.disabled = disabled;
            })
            .unwrap();
            assert!(!h
                .dispatch(WebEvent::Insert {
                    id: "password".into(),
                    text: "blocked".into(),
                    paste: false
                })
                .unwrap());
            assert_eq!(h.context().password, "new-private-🔐");
        }
    }
    #[test]
    fn public_selection_uses_valid_utf8_and_ime_drafts_do_not_commit() {
        let mut h = host();
        h.dispatch(WebEvent::Focus { id: "name".into() }).unwrap();
        h.dispatch(WebEvent::EditorValue {
            id: "name".into(),
            text: "A🌿é".into(),
            anchor: 1,
            caret: 5,
        })
        .unwrap();
        assert_eq!(h.runtime.editor_selection("name"), Some((1, 5)));
        assert!(!h
            .dispatch(WebEvent::Select {
                id: "name".into(),
                anchor: 2,
                caret: 4
            })
            .unwrap());
        h.dispatch(WebEvent::Composition {
            id: "name".into(),
            phase: "update".into(),
            text: "東京".into(),
        })
        .unwrap();
        assert_eq!(h.context().text, "A🌿é");
        assert!(!h.snapshot().to_string().contains("東京"));
        h.dispatch(WebEvent::Composition {
            id: "name".into(),
            phase: "commit".into(),
            text: "東京".into(),
        })
        .unwrap();
        assert_eq!(h.context().text, "A東京é");
        h.dispatch(WebEvent::Composition {
            id: "name".into(),
            phase: "update".into(),
            text: "draft".into(),
        })
        .unwrap();
        h.dispatch(WebEvent::Focus {
            id: "password".into(),
        })
        .unwrap();
        assert!(!h
            .dispatch(WebEvent::Composition {
                id: "name".into(),
                phase: "commit".into(),
                text: "late".into()
            })
            .unwrap());
        assert_eq!(h.context().text, "A東京é");
    }
    #[test]
    fn wire_errors_do_not_echo_private_input() {
        let error = WebEvent::decode(
            br#"{"op":"insert","id":"password","text":"super-secret","unexpected":true}"#,
        )
        .err()
        .unwrap();
        assert_eq!(error, "invalid browser event");
    }
    #[test]
    fn suspension_blocks_mutations_and_resume_preserves_state() {
        let mut h = host();
        h.dispatch(WebEvent::Suspend { value: true }).unwrap();
        assert!(!h.dispatch(WebEvent::Activate { id: "add".into() }).unwrap());
        assert!(!h.advance(1000.).unwrap());
        h.dispatch(WebEvent::Suspend { value: false }).unwrap();
        h.dispatch(WebEvent::Activate { id: "add".into() }).unwrap();
        assert_eq!(h.context().count, 1);
    }
    #[test]
    fn physical_pointer_mapping_scroll_and_bounded_resize_match_shared_runtime() {
        let mut h = host();
        h.dispatch(WebEvent::Resize {
            width: 500.,
            height: 600.,
            scale: 2.,
        })
        .unwrap();
        let node = h
            .runtime
            .accessibility_nodes()
            .into_iter()
            .find(|n| n.id == "add")
            .unwrap();
        let [x, y, w, hh] = node.bounds;
        let (x, y) = ((x + w / 2.) / 2., (y + hh / 2.) / 2.);
        h.dispatch(WebEvent::Down { x, y }).unwrap();
        h.dispatch(WebEvent::Up { x, y }).unwrap();
        assert_eq!(h.context().count, 1);
        let node = h
            .runtime
            .accessibility_nodes()
            .into_iter()
            .find(|n| n.id == "list")
            .unwrap();
        let [x, y, w, hh] = node.bounds;
        assert!(h
            .dispatch(WebEvent::Wheel {
                x: (x + w / 2.) / 2.,
                y: (y + hh / 2.) / 2.,
                dx: 0.,
                dy: 80.
            })
            .unwrap());
        assert!(h.runtime.scroll_offset("list")[1] > 0.);
        assert!(h
            .dispatch(WebEvent::Resize {
                width: 9000.,
                height: 8000.,
                scale: 4.
            })
            .is_err());
        assert!(h.advance(f64::NAN).is_err());
    }
    #[test]
    fn software_frame_has_actual_pixels_and_correct_extent() {
        let mut h = host();
        h.dispatch(WebEvent::Resize {
            width: 320.,
            height: 400.,
            scale: 1.,
        })
        .unwrap();
        let (pixels, w, hh) = h.rgba(&UiTheme::light()).unwrap();
        assert_eq!((w, hh), (320, 400));
        assert_eq!(pixels.len(), 320 * 400 * 4);
        assert!(pixels.chunks_exact(4).any(|p| p[0] < 240));
    }
}
