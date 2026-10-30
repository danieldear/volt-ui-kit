//! A real Rust UI shared by native tests and the browser documentation.
use volt_ui_kit::{ui::prelude::*, ColorToken};
// Native OS adapters are cfg-gated inside this shared example module.
#[allow(dead_code)]
#[path = "../../catalog/mod.rs"]
mod catalog;
mod editor;

#[derive(Default)]
pub struct Demo {
    count: u32,
    tab: u8,
    name: String,
    saved: String,
    dark: bool,
    checked: bool,
    password: String,
    editor: editor::Editor,
    editor_readonly: bool,
}
pub fn view(app: &Demo) -> Element<Demo> {
    let header = stack![
        vstack![
            text("VOLT / LIVE COMPONENTS")
                .font_size(11.)
                .font_weight(600)
                .color(ColorToken::Muted),
            text("Rust. In your browser.")
                .responsive_font_size(responsive(22.).at(Breakpoint::Md, 27.))
                .font_weight(700)
        ]
        .gap(5., Axis::Vertical),
        spacer(),
        button(if app.dark { "Light mode" } else { "Dark mode" })
            .key("appearance")
            .variant(ButtonVariant::Outline)
            .on_click(|a: &mut Demo| a.dark = !a.dark),
    ]
    .align(Align::Start)
    .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
    .gap(12., Axis::Both);
    let tabs = tab_bar([
        tab_item(0u8, "Components"),
        tab_item(1, "Editable form"),
        tab_item(2, "Scrolling"),
        tab_item(3, "Editor"),
    ])
    .key("demo-tabs")
    .accessible_label("Demo pages")
    .selected(app.tab)
    .on_select(|a: &mut Demo, t| a.tab = t);
    let content: Element<Demo> = match app.tab {
        0 => vstack![
            text("Not a screenshot.").font_size(22.).font_weight(600),
            text("These are the same Volt builders, layout and Rust callbacks used on desktop.")
                .font_size(14.)
                .color(ColorToken::Muted),
            stack![
                button("Add one")
                    .key("increment")
                    .variant(ButtonVariant::Primary)
                    .on_click(|a: &mut Demo| a.count += 1),
                button("Reset")
                    .key("reset")
                    .variant(ButtonVariant::Outline)
                    .on_click(|a: &mut Demo| a.count = 0),
                text(format!("Count: {}", app.count))
                    .key("count")
                    .font_size(18.)
                    .font_weight(600)
            ]
            .gap(12., Axis::Both)
            .align(Align::Center)
            .direction(responsive(Direction::Vertical).at(Breakpoint::Sm, Direction::Horizontal)),
            checkbox("Enable notifications", app.checked)
                .key("check")
                .on_change(|a: &mut Demo, v| a.checked = v),
            text(if app.checked {
                "Enabled locally — no OS notification is sent."
            } else {
                "Try pointer, keyboard, themes and the other tabs."
            })
            .color(ColorToken::Muted),
        ]
        .gap(18., Axis::Vertical)
        .into(),
        1 => vstack![
            text("An editable Rust form.")
                .font_size(22.)
                .font_weight(600),
            text("Display name").font_weight(600),
            text_field(&app.name)
                .key("name")
                .accessible_label("Display name")
                .placeholder("Your name")
                .on_change(|a: &mut Demo, v| a.name = v)
                .fill_width(),
            button("Save profile")
                .key("save")
                .on_click(|a: &mut Demo| a.saved = a.name.trim().into()),
            text(if app.saved.is_empty() {
                "Changes stay in this demo only.".into()
            } else {
                format!("Saved: {}", app.saved)
            })
            .key("saved")
            .color(ColorToken::Muted),
            text_field(&app.password)
                .key("password")
                .password(true)
                .accessible_label("Password")
                .placeholder("Write-only password input")
                .on_change(|a: &mut Demo, s| a.password = s),
            text(if app.password.is_empty() {
                "Password stays in Rust."
            } else {
                "Password updated in Rust."
            })
            .key("password-status")
            .font_size(12.),
        ]
        .gap(14., Axis::Vertical)
        .into(),
        3 => editor::view(app),
        _ => vstack((1..=40).map(|i| -> Element<Demo> {
            hstack![
                text(format!("{i:02}")).font_weight(600).width(32.),
                text(format!("Rust-native item {i}")),
                spacer(),
                text("Volt UI").color(ColorToken::Muted)
            ]
            .padding(14., Edge::All)
            .border_width(1.)
            .border_color(ColorToken::Border)
            .rounded(8.)
            .into()
        }))
        .key("scroll-list")
        .accessible_label("Scrollable list")
        .height(220.)
        .scroll(Scroll::Vertical)
        .gap(8., Axis::Vertical)
        .into(),
    };
    vstack![
        header,
        tabs,
        panel![content]
            .padding(22., Edge::All)
            .background(ColorToken::Base200)
            .rounded(12.)
            .fill_width(),
        text("Native components · shared Rust runtime · no replacement HTML UI")
            .font_size(11.)
            .color(ColorToken::Muted)
    ]
    .key("demo-root")
    .scroll(Scroll::Vertical)
    .font_family("Inter")
    .padding(responsive(16.).at(Breakpoint::Sm, 24.), Edge::All)
    .gap(20., Axis::Vertical)
    .fill_width()
    .fill_height()
    .background(ColorToken::Base100)
    .into()
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::sync::OnceLock;
    use volt_ui_kit::{rendering::register_font, UiTheme};
    use volt_ui_kit_web::{browser::BrowserHost, WebEvent, WebView};
    use wasm_bindgen::prelude::*;
    static FONT: OnceLock<Result<(), String>> = OnceLock::new();
    fn register_browser_fonts() -> Result<(), String> {
        for bytes in [
            include_bytes!("../../../docs/live/InterVariable.ttf").as_slice(),
            include_bytes!("../../../docs/live/InterVariable-Italic.ttf").as_slice(),
            include_bytes!("../../../docs/live/JetBrainsMono.ttf").as_slice(),
        ] {
            register_font(bytes.to_vec())?;
        }
        Ok(())
    }
    fn browser_diagnostics() {
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(|| {
            std::panic::set_hook(Box::new(|info| {
                web_sys::console::error_1(&info.to_string().into())
            }))
        });
    }
    #[wasm_bindgen]
    pub struct CatalogDemo {
        host: BrowserHost<Demo>,
    }
    #[wasm_bindgen]
    impl CatalogDemo {
        #[wasm_bindgen(js_name=mount)]
        pub async fn mount(
            canvas: web_sys::HtmlCanvasElement,
            gpu: bool,
            dark: bool,
            page: u8,
        ) -> Result<CatalogDemo, JsValue> {
            browser_diagnostics();
            FONT.get_or_init(register_browser_fonts)
                .clone()
                .map_err(|e| JsValue::from_str(&e))?;
            let host = WebView::new(
                Demo {
                    dark,
                    tab: page.min(3),
                    ..Demo::default()
                },
                view,
            )
            .map_err(|e| JsValue::from_str(&e))?;
            Ok(Self {
                host: BrowserHost::new(canvas, host, gpu).await?,
            })
        }
        pub fn dispatch(&mut self, json: &str) -> Result<bool, JsValue> {
            let event = WebEvent::decode(json.as_bytes()).map_err(|e| JsValue::from_str(&e))?;
            self.host
                .view
                .dispatch(event)
                .map_err(|e| JsValue::from_str(&e))
        }
        pub fn clipboard(&mut self, cut: bool) -> Result<Option<String>, JsValue> {
            self.host
                .view
                .clipboard(cut)
                .map_err(|e| JsValue::from_str(&e))
        }
        pub fn snapshot(&self) -> String {
            self.host.snapshot()
        }
        pub fn frame(&mut self, ms: f64) -> Result<bool, JsValue> {
            self.host
                .view
                .advance(ms)
                .map_err(|e| JsValue::from_str(&e))?;
            let theme = if self.host.view.context().dark {
                UiTheme::from_palette(volt_ui_kit::Palette::zinc_dark())
            } else {
                UiTheme::from_palette(volt_ui_kit::Palette::zinc_light())
            };
            self.host.render(&theme)
        }
    }
    /// An original native Catalog section, mounted with the same Rust state and callbacks.
    #[wasm_bindgen]
    pub struct CatalogPreview {
        host: BrowserHost<catalog::App>,
        system_dark: bool,
    }
    #[wasm_bindgen]
    impl CatalogPreview {
        pub async fn mount(
            canvas: web_sys::HtmlCanvasElement,
            gpu: bool,
            dark: bool,
            page: &str,
            section: &str,
            accent: usize,
        ) -> Result<CatalogPreview, JsValue> {
            browser_diagnostics();
            FONT.get_or_init(register_browser_fonts)
                .clone()
                .map_err(|e| JsValue::from_str(&e))?;
            let app = catalog::browser::app(page, section, dark, accent)
                .map_err(|e| JsValue::from_str(&e))?;
            let view =
                WebView::new(app, catalog::browser::view).map_err(|e| JsValue::from_str(&e))?;
            Ok(Self {
                host: BrowserHost::new(canvas, view, gpu).await?,
                system_dark: dark,
            })
        }
        pub fn dispatch(&mut self, json: &str) -> Result<bool, JsValue> {
            self.host
                .view
                .dispatch(WebEvent::decode(json.as_bytes()).map_err(|e| JsValue::from_str(&e))?)
                .map_err(|e| JsValue::from_str(&e))
        }
        pub fn clipboard(&mut self, cut: bool) -> Result<Option<String>, JsValue> {
            self.host
                .view
                .clipboard(cut)
                .map_err(|e| JsValue::from_str(&e))
        }
        pub fn snapshot(&self) -> String {
            self.host.snapshot()
        }
        pub fn frame(&mut self, ms: f64) -> Result<bool, JsValue> {
            self.host
                .view
                .advance(ms)
                .map_err(|e| JsValue::from_str(&e))?;
            let theme = catalog::theme(
                self.host.view.context(),
                if self.system_dark {
                    volt_ui_kit::ThemeMode::Dark
                } else {
                    volt_ui_kit::ThemeMode::Light
                },
            );
            self.host.render(&theme)
        }
        pub fn appearance(&mut self, dark: bool, accent: usize) -> Result<(), JsValue> {
            self.system_dark = dark;
            if accent >= catalog::THEMES.len() {
                return Err(JsValue::from_str("unknown accent"));
            }
            self.host
                .view
                .update(|app| {
                    app.mode = if dark {
                        volt_ui_kit::ThemeMode::Dark
                    } else {
                        volt_ui_kit::ThemeMode::Light
                    };
                    app.theme = accent;
                    app.custom_palettes = None;
                })
                .map_err(|e| JsValue::from_str(&e))
        }
        pub fn pause(&mut self) {
            self.host.pause();
        }
        pub async fn resume(
            &mut self,
            canvas: web_sys::HtmlCanvasElement,
            gpu: bool,
        ) -> Result<(), JsValue> {
            self.host.resume(canvas, gpu).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit_web::{WebEvent, WebView};
    #[test]
    fn code_ime_is_application_owned_and_read_only_blocks_mutations() {
        use volt_ui_kit::declarative::CodeInput;
        let mut host = WebView::new(
            Demo {
                tab: 3,
                ..Demo::default()
            },
            view,
        )
        .unwrap();
        host.dispatch(WebEvent::Focus { id: "code".into() })
            .unwrap();
        let original = host.context().editor.text.clone();
        host.dispatch(WebEvent::Composition {
            id: "code".into(),
            phase: "update".into(),
            text: "東京🌿".into(),
        })
        .unwrap();
        assert_eq!(host.context().editor.text, original);
        assert!(!host.snapshot().to_string().contains("東京"));
        host.dispatch(WebEvent::Composition {
            id: "code".into(),
            phase: "commit".into(),
            text: "東京🌿".into(),
        })
        .unwrap();
        assert_eq!(host.context().editor.text, format!("東京🌿{original}"));
        assert_eq!(host.context().editor.changes, 1);
        host.update(|a| a.editor_readonly = true).unwrap();
        assert!(!host
            .dispatch(WebEvent::Insert {
                id: "code".into(),
                text: "blocked".into(),
                paste: true
            })
            .unwrap());
        assert_eq!(host.context().editor.changes, 1);
        host.update(|a| {
            a.editor.input(CodeInput::Key {
                key: volt_ui_kit::declarative::CodeKey::SelectAll,
                modifiers: Default::default(),
            });
        })
        .unwrap();
        assert_eq!(
            host.clipboard(true).unwrap().as_deref(),
            Some(host.context().editor.text.as_str())
        );
        assert_eq!(
            host.context().editor.changes,
            1,
            "read-only cut is copy, never a mutation"
        );
    }
    #[test]
    fn every_demo_page_builds_and_form_callbacks_keep_unicode() {
        for tab in 0..4 {
            WebView::new(
                Demo {
                    tab,
                    ..Demo::default()
                },
                view,
            )
            .unwrap();
        }
        let mut h = WebView::new(
            Demo {
                tab: 1,
                ..Demo::default()
            },
            view,
        )
        .unwrap();
        h.dispatch(WebEvent::Text {
            id: "name".into(),
            text: "Zoë 東京".into(),
        })
        .unwrap();
        h.dispatch(WebEvent::Activate { id: "save".into() })
            .unwrap();
        assert_eq!(h.context().saved, "Zoë 東京");
    }
}
