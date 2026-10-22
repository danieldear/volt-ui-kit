#![cfg(feature = "rust-ui")]
//! The code view draws only the lines on screen, reports what it needs, and
//! hands every edit to the application.
use volt_ui_kit::{
    declarative::{
        Bindings, CodeDecoration, CodeInput, CodeKey, CodeLine, CodeMark, CodePosition,
        CodeSelection, DocumentPainter, Handlers, InputModifiers, Runtime, TextMetrics, TextStyle,
    },
    ui::prelude::*,
    Painter, Viewport,
};

#[derive(Default)]
struct Doc {
    lines: Vec<String>,
    first: usize,
    visible: Option<(usize, usize)>,
    inputs: Vec<CodeInput>,
    selection: Option<CodeSelection>,
    reveal: u32,
    scroll: Option<(usize, u32)>,
    read_only: bool,
    palette: Option<String>,
    /// Lay a transparent row holding a solid bar over the view.
    overlay: bool,
    focus: bool,
    /// Watch this many points at the bottom for the caret.
    band: Option<f32>,
    decorations: Vec<CodeDecoration>,
    /// Indentation guides every this many columns, palette 0 in italic, and
    /// lines marked along the scrollbar.
    guides: Option<usize>,
    italic: bool,
    marks: Vec<(usize, String)>,
}
impl Doc {
    fn new(lines: usize) -> Self {
        Self {
            lines: (0..lines).map(|i| format!("line {i} = value;")).collect(),
            ..Self::default()
        }
    }
}
/// The window the application supplies: 40 lines either side of the screen.
const OVERSCAN: usize = 40;

fn view(d: &Doc) -> Element<Doc> {
    let end = (d.first + 120).min(d.lines.len());
    let lines: Vec<CodeLine> = d.lines[d.first..end]
        .iter()
        .map(|t| CodeLine {
            text: t.clone(),
            spans: vec![(0, 4.min(t.len() as u32), 0)],
        })
        .collect();
    let mut v = code_view(d.lines.len())
        .key("code")
        .lines(d.first, lines)
        .palette([d.palette.clone().unwrap_or_else(|| "#ff0000".into())])
        .focus_when(d.focus)
        .selections(
            vec![d
                .selection
                .unwrap_or(CodeSelection::caret(CodePosition::default()))],
            0,
        )
        .reveal(d.reveal)
        .read_only(d.read_only)
        .decorations(d.decorations.clone())
        .scroll_marks(d.marks.clone())
        .italic_palette(d.italic.then_some(0))
        .on_input(|d: &mut Doc, input| {
            if let CodeInput::Visible { first, .. } = &input {
                d.visible = match &input {
                    CodeInput::Visible { first, last } => Some((*first, *last)),
                    _ => None,
                };
                d.first = first.saturating_sub(OVERSCAN);
            }
            let reply = matches!(input, CodeInput::Copy | CodeInput::Cut).then(|| "copied".into());
            d.inputs.push(input);
            reply
        })
        .width(600.)
        .height(400.);
    if let Some((line, request)) = d.scroll {
        v = v.scroll_to(line, request);
    }
    if let Some(height) = d.band {
        v = v.caret_band(height);
    }
    if let Some(unit) = d.guides {
        v = v.indent_guides(unit);
    }
    if !d.overlay {
        return v.into();
    }
    let bar = panel([])
        .key("bar")
        .width(200.)
        .height(40.)
        .background("#202020");
    let layer = hstack([spacer().into(), bar.into()])
        .key("layer")
        .align(Align::Start);
    zstack([v.into(), layer.into()])
        .key("stack")
        .align(Align::Stretch)
        .width(600.)
        .height(400.)
        .into()
}

/// Monospace metrics (half the font size per character) that record text.
#[derive(Default)]
struct Recorder {
    texts: Vec<String>,
    /// Each styled text's run colours.
    runs: Vec<(String, Vec<[f32; 4]>)>,
    /// Each styled text's runs drawn in italic.
    italics: Vec<(String, Vec<bool>)>,
    /// Every rectangle drawn, in order: bounds and colour.
    rects: std::cell::RefCell<Vec<([f32; 4], [f32; 4])>>,
}
impl TextMetrics for Recorder {
    fn measure(&mut self, t: &str, size: f32, w: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(w), size * 1.2]
    }
}
impl Painter for Recorder {
    type Shape = ();
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600., 400., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.rects.borrow_mut().push(([x, y, w, h], color));
    }
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, _: &mut Vec<()>, text: &str, _: [f32; 4], _: f32, _: [f32; 4]) {
        self.texts.push(text.into());
    }
    fn ui_text_styled(
        &mut self,
        _: &mut Vec<()>,
        text: &str,
        _: [f32; 4],
        style: &TextStyle,
        _: [f32; 4],
    ) {
        self.texts.push(text.into());
        let colors = style.runs.iter().filter_map(|r| r.color).collect();
        self.runs.push((text.into(), colors));
        let italic = style.runs.iter().map(|r| r.italic == Some(true)).collect();
        self.italics.push((text.into(), italic));
    }
}

struct Mounted {
    runtime: Runtime,
    handlers: Handlers<'static, Doc>,
    painter: Recorder,
}
impl Mounted {
    fn new(doc: &mut Doc) -> Self {
        let (d, handlers) = view(doc).build().unwrap().into_parts();
        let runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
        let mut m = Self {
            runtime,
            handlers,
            painter: Recorder::default(),
        };
        m.settle(doc);
        m
    }
    /// Lay out, let views report, recompose until nothing changes.
    fn settle(&mut self, doc: &mut Doc) {
        for _ in 0..8 {
            let vp = self.painter.viewport();
            self.runtime
                .layout(vp, &Bindings::default(), &mut self.painter)
                .unwrap();
            if !self
                .runtime
                .dispatch_widths(&mut self.handlers, doc)
                .unwrap()
            {
                return;
            }
            self.rebuild(doc);
        }
        panic!("code view kept reporting");
    }
    fn rebuild(&mut self, doc: &mut Doc) {
        let (d, h) = view(doc).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &h, &Bindings::default())
            .unwrap();
        self.handlers = h;
    }
    fn paint(&mut self) -> Vec<String> {
        self.painter.texts.clear();
        let _ = self.runtime.paint_ui(&mut self.painter, &UiTheme::dark());
        std::mem::take(&mut self.painter.texts)
    }
    /// The span colours drawn for line 0 under `theme`.
    fn first_line_colors(&mut self, theme: &UiTheme) -> Vec<[f32; 4]> {
        self.painter.runs.clear();
        let _ = self.runtime.paint_ui(&mut self.painter, theme);
        let runs = std::mem::take(&mut self.painter.runs);
        runs.into_iter()
            .find(|(text, _)| text == "line 0 = value;")
            .map(|(_, colors)| colors)
            .unwrap()
    }
    fn bounds(&self) -> [f32; 4] {
        self.bounds_of("code")
    }
    fn bounds_of(&self, key: &str) -> [f32; 4] {
        self.runtime
            .boxes()
            .iter()
            .find(|b| b.id == key)
            .unwrap()
            .bounds
            .bounds()
    }
}

#[test]
fn a_view_needs_a_key_an_input_handler_and_a_valid_window() {
    let missing_key: Element<Doc> = code_view(1).on_input(|_, _| None).into();
    assert!(missing_key.build().is_err());
    let missing_input: Element<Doc> = code_view(1).key("c").into();
    assert!(missing_input.build().is_err());
    let past_end: Element<Doc> = code_view(2)
        .key("c")
        .lines(1, vec![CodeLine::plain("a"), CodeLine::plain("b")])
        .on_input(|_, _| None)
        .into();
    assert!(past_end.build().is_err());
}

#[test]
fn only_the_visible_lines_of_a_huge_document_are_drawn() {
    let mut doc = Doc::new(1_000_000);
    let mut m = Mounted::new(&mut doc);
    // 400 points at 20 per line: lines 0..20 are on screen.
    assert_eq!(doc.visible, Some((0, 20)));
    let texts = m.paint();
    let code: Vec<_> = texts.iter().filter(|t| t.starts_with("line ")).collect();
    assert_eq!(code.len(), 20);
    assert_eq!(code[0], "line 0 = value;");
    // The gutter numbers the same lines, sized for seven digits.
    assert!(texts.contains(&"20".to_string()));
    assert!(!texts.contains(&"21".to_string()));
}

#[test]
fn scrolling_moves_the_text_and_asks_for_lines_only_outside_the_supplied_window() {
    let mut doc = Doc::new(10_000);
    let mut m = Mounted::new(&mut doc);
    let [x, y, w, h] = m.bounds();
    let reports = doc.inputs.len();
    // Within the 120 supplied lines: no recomposition is needed.
    let outcome = m
        .runtime
        .scroll_outcome(
            x + w / 2.,
            y + h / 2.,
            [0., 400.],
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    assert!(outcome.changed && !outcome.activated);
    assert_eq!(doc.inputs.len(), reports);
    assert_eq!(m.runtime.code_view_offset("code"), Some([0., 400.]));
    let texts = m.paint();
    assert!(texts.contains(&"line 20 = value;".to_string()));
    assert!(!texts.contains(&"line 19 = value;".to_string()));
    // Far beyond them, the view reports the new window.
    let outcome = m
        .runtime
        .scroll_outcome(
            x + w / 2.,
            y + h / 2.,
            [0., 16_000.],
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    assert!(outcome.activated);
    assert_eq!(doc.visible, Some((820, 840)));
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert!(m.paint().contains(&"line 820 = value;".to_string()));
}

#[test]
fn a_press_focuses_the_view_and_reports_line_and_column() {
    let mut doc = Doc::new(100);
    let mut m = Mounted::new(&mut doc);
    let [x, y, _, _] = m.bounds();
    m.paint();
    // Text starts after the 64-point gutter and a 12-point inset; at 13 pt
    // each column is 6.5 points wide.
    let (px, py) = (x + 64. + 12. + 6.5 * 5., y + 20. * 3. + 10.);
    let outcome = m
        .runtime
        .code_pointer_down(
            px,
            py,
            2,
            InputModifiers::default(),
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    assert!(outcome.is_some_and(|o| o.activated));
    assert!(m.runtime.code_view_focused());
    assert_eq!(
        doc.inputs.last(),
        Some(&CodeInput::Press {
            at: CodePosition::new(3, 5),
            clicks: 2,
            modifiers: InputModifiers::default(),
        })
    );
    m.runtime
        .code_pointer_drag(px + 6.5 * 3., py + 20., &mut m.handlers, &mut doc)
        .unwrap();
    assert_eq!(
        doc.inputs.last(),
        Some(&CodeInput::Drag {
            at: CodePosition::new(4, 8)
        })
    );
    assert!(m.runtime.code_pointer_up());
    assert!(m
        .runtime
        .code_pointer_drag(px, py, &mut m.handlers, &mut doc)
        .unwrap()
        .is_none());
    // Outside any code view the host handles the press itself.
    assert!(m
        .runtime
        .code_pointer_down(
            x - 1.,
            y - 1.,
            1,
            InputModifiers::default(),
            &mut m.handlers,
            &mut doc
        )
        .unwrap()
        .is_none());
}

#[test]
fn keys_and_text_go_to_the_focused_view_and_read_only_views_only_copy() {
    let mut doc = Doc::new(10);
    let mut m = Mounted::new(&mut doc);
    let [x, y, _, _] = m.bounds();
    m.runtime
        .code_pointer_down(
            x + 100.,
            y + 5.,
            1,
            InputModifiers::default(),
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    m.runtime.code_pointer_up();
    let shift = InputModifiers {
        shift: true,
        ..InputModifiers::default()
    };
    let key = CodeInput::Key {
        key: CodeKey::PageDown,
        modifiers: shift,
    };
    m.runtime
        .code_input(key.clone(), &mut m.handlers, &mut doc)
        .unwrap();
    m.runtime
        .code_input(CodeInput::Insert("é".into()), &mut m.handlers, &mut doc)
        .unwrap();
    let (_, copied) = m
        .runtime
        .code_input(CodeInput::Copy, &mut m.handlers, &mut doc)
        .unwrap();
    assert_eq!(copied.as_deref(), Some("copied"));
    assert_eq!(
        &doc.inputs[doc.inputs.len() - 3..],
        &[key, CodeInput::Insert("é".into()), CodeInput::Copy]
    );

    doc.read_only = true;
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    let before = doc.inputs.len();
    let (outcome, _) = m
        .runtime
        .code_input(CodeInput::Insert("x".into()), &mut m.handlers, &mut doc)
        .unwrap();
    assert!(!outcome.activated);
    assert_eq!(doc.inputs.len(), before);
    let (_, copied) = m
        .runtime
        .code_input(CodeInput::Cut, &mut m.handlers, &mut doc)
        .unwrap();
    assert_eq!(copied.as_deref(), Some("copied"));
    assert_eq!(doc.inputs.last(), Some(&CodeInput::Copy));
    assert!(!m.runtime.editing_ime_allowed());
}

#[test]
fn input_method_composition_is_visual_until_committed() {
    let mut doc = Doc::new(10);
    let mut m = Mounted::new(&mut doc);
    let [x, y, _, _] = m.bounds();
    doc.selection = Some(CodeSelection::caret(CodePosition::new(2, 4)));
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    m.runtime
        .code_pointer_down(
            x + 100.,
            y + 5.,
            1,
            InputModifiers::default(),
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    m.runtime.code_pointer_up();
    assert!(m.runtime.editing_ime_allowed());
    let before = doc.inputs.len();
    assert!(m.runtime.ime_preedit("にほ", Some((0, 6))));
    assert!(m.runtime.ime_active());
    assert_eq!(doc.inputs.len(), before);
    // The composition is drawn inside line 2 at the caret.
    assert!(m.paint().contains(&"lineにほ 2 = value;".to_string()));
    let [cx, cy, _, ch] = m.runtime.editor_candidate_bounds().unwrap();
    assert_eq!((cx, cy, ch), (x + 64. + 12. + 4. * 6.5, y + 40., 20.));
    m.runtime
        .ime_commit("日本", &mut m.handlers, &mut doc)
        .unwrap();
    assert_eq!(doc.inputs.last(), Some(&CodeInput::Insert("日本".into())));
    assert!(!m.runtime.ime_active());
}

#[test]
fn reveal_and_scroll_requests_bring_lines_into_view() {
    let mut doc = Doc::new(5000);
    let mut m = Mounted::new(&mut doc);
    // A caret far below: revealing scrolls just enough to show its line.
    doc.selection = Some(CodeSelection::caret(CodePosition::new(300, 0)));
    doc.reveal = 1;
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert_eq!(
        m.runtime.code_view_offset("code"),
        Some([0., 301. * 20. - 400.])
    );
    assert_eq!(doc.visible, Some((281, 301)));
    // The same request number does nothing more.
    let [x, y, w, h] = m.bounds();
    m.runtime
        .scroll_outcome(
            x + w / 2.,
            y + h / 2.,
            [0., -100.],
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert_eq!(
        m.runtime.code_view_offset("code"),
        Some([0., 301. * 20. - 500.])
    );
    // Go to line: a third of the way down.
    doc.scroll = Some((2000, 1));
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert_eq!(
        m.runtime.code_view_offset("code"),
        Some([0., 2000. * 20. - 400. / 3.])
    );
    assert!(m.paint().contains(&"line 2000 = value;".to_string()));
}

#[test]
fn light_and_dark_palette_pairs_follow_the_theme_in_effect() {
    let mut doc = Doc::new(5);
    doc.palette = Some("#112233|#DDEEFF".into());
    let mut m = Mounted::new(&mut doc);
    let rgb = |r: u8, g: u8, b: u8| [r as f32 / 255., g as f32 / 255., b as f32 / 255., 1.];
    assert_eq!(
        m.first_line_colors(&UiTheme::dark()),
        vec![rgb(0xDD, 0xEE, 0xFF)]
    );
    assert_eq!(
        m.first_line_colors(&UiTheme::light()),
        vec![rgb(0x11, 0x22, 0x33)]
    );
    // A plain entry is the same in both.
    doc.palette = Some("#FF0000".into());
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert_eq!(m.first_line_colors(&UiTheme::dark()), vec![rgb(0xFF, 0, 0)]);
    assert_eq!(
        m.first_line_colors(&UiTheme::light()),
        vec![rgb(0xFF, 0, 0)]
    );
}

#[test]
fn presses_pass_through_transparent_layers_but_not_solid_ones() {
    let mut doc = Doc::new(50);
    doc.overlay = true;
    let mut m = Mounted::new(&mut doc);
    m.paint();
    let none = InputModifiers::default();
    // The bar sits top-right on a full-size transparent row.
    let [bx, by, bw, bh] = m.bounds_of("bar");
    let hit_bar = m
        .runtime
        .code_pointer_down(
            bx + bw / 2.,
            by + bh / 2.,
            1,
            none,
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    assert!(hit_bar.is_none(), "the solid bar keeps its press");
    assert!(!m.runtime.code_view_focused());
    let [x, y, _, _] = m.bounds();
    let hit_code = m
        .runtime
        .code_pointer_down(x + 100., y + 200., 1, none, &mut m.handlers, &mut doc)
        .unwrap();
    assert!(
        hit_code.is_some_and(|o| o.activated),
        "the empty layer lets it through"
    );
    assert!(matches!(doc.inputs.last(), Some(CodeInput::Press { .. })));
    m.runtime.code_pointer_up();
    // Scrolling passes through the same way.
    assert!(m.runtime.scroll_at(x + 100., y + 200., [0., 40.]));
    assert_eq!(m.runtime.code_view_offset("code"), Some([0., 40.]));
}

#[test]
fn focus_when_takes_focus_as_it_turns_on() {
    let mut doc = Doc::new(5);
    let mut m = Mounted::new(&mut doc);
    assert!(!m.runtime.code_view_focused());
    doc.focus = true;
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert!(m.runtime.code_view_focused());
    assert!(m.runtime.editing_ime_allowed());
}

#[test]
fn the_caret_band_reports_when_the_caret_enters_and_leaves_the_bottom_of_the_view() {
    let mut doc = Doc::new(200);
    doc.band = Some(60.);
    let mut m = Mounted::new(&mut doc);
    m.paint();
    let band = |doc: &Doc| {
        doc.inputs.iter().rev().find_map(|i| match i {
            CodeInput::CaretBand(span) => Some(*span),
            _ => None,
        })
    };
    // Line 0 is far from the bottom 60 points (lines 17 to 19).
    assert_eq!(band(&doc), Some(None));
    let [x, _, _, _] = m.bounds();
    doc.selection = Some(CodeSelection::caret(CodePosition::new(18, 4)));
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    // Text starts after the 64-point gutter and a 12-point inset; at 13 pt a
    // column is 6.5 points wide.
    let expected = (x + 64. + 12. + 6.5 * 4.).round() as i32;
    assert_eq!(band(&doc), Some(Some([expected, 7])));
    // Unchanged geometry is not reported again.
    let reports = doc.inputs.len();
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    assert_eq!(doc.inputs.len(), reports);
    // Scrolling the caret line up out of the band reports that it left.
    let [bx, by, bw, bh] = m.bounds();
    m.runtime
        .scroll_outcome(
            bx + bw / 2.,
            by + bh / 2.,
            [0., 100.],
            &mut m.handlers,
            &mut doc,
        )
        .unwrap();
    m.runtime
        .dispatch_widths(&mut m.handlers, &mut doc)
        .unwrap();
    assert_eq!(band(&doc), Some(None));
}

#[test]
fn decorations_fill_outline_underline_and_tint_lines_by_mark() {
    let at = CodePosition::new;
    let mut doc = Doc::new(200);
    doc.decorations = vec![
        CodeDecoration::new(at(1, 4), at(1, 0), CodeMark::Fill, "#00FF00"),
        CodeDecoration::new(at(2, 5), at(2, 6), CodeMark::Outline, "#0000FF|#FF00FF"),
        CodeDecoration::new(at(3, 0), at(3, 4), CodeMark::Underline, "#FFFF00"),
        CodeDecoration::new(at(4, 0), at(5, 0), CodeMark::Lines, "#00FFFF40"),
        CodeDecoration::new(at(7, 0), at(7, 0), CodeMark::Lines, "#FF000040"),
        // Empty ranges and ranges far off screen draw nothing.
        CodeDecoration::new(at(1, 2), at(1, 2), CodeMark::Fill, "#123456"),
        CodeDecoration::new(at(150, 0), at(150, 3), CodeMark::Fill, "#654321"),
    ];
    let mut m = Mounted::new(&mut doc);
    m.painter.rects.borrow_mut().clear();
    m.paint();
    let rects = m.painter.rects.take();
    let [bx, by, ..] = m.bounds();
    let rgba = |r: u8, g: u8, b: u8, a: u8| [r, g, b, a].map(|v| v as f32 / 255.);
    let drawn = |color: [f32; 4]| -> Vec<[f32; 4]> {
        rects
            .iter()
            .filter(|(_, c)| *c == color)
            .map(|(r, _)| *r)
            .collect()
    };
    // Monospace metrics: 6.5pt per column; text starts after a 64pt gutter
    // and a 12pt inset; lines are 20pt.
    let (text_x, col) = (bx + 76., 6.5);
    assert_eq!(
        drawn(rgba(0, 255, 0, 255)),
        vec![[text_x, by + 20., 4. * col, 20.]]
    );
    // The dark half of a light|dark pair; four sides around column 5.
    let outline = drawn(rgba(255, 0, 255, 255));
    assert_eq!(outline.len(), 4);
    assert!(outline.contains(&[text_x + 5. * col, by + 40., col, 1.]));
    assert!(outline.contains(&[text_x + 5. * col, by + 40., 1., 20.]));
    assert_eq!(
        drawn(rgba(255, 255, 0, 255)),
        vec![[text_x, by + 78., 4. * col, 1.]]
    );
    // Two whole lines, each with a tint across the view and a solid bar.
    let tint = drawn(rgba(0, 255, 255, 0x40));
    assert_eq!(
        tint,
        vec![[bx, by + 80., 600., 20.], [bx, by + 100., 600., 20.]]
    );
    assert_eq!(drawn(rgba(0, 255, 255, 255)).len(), 2);
    // An empty range marks removed lines: a hairline on the boundary above
    // line 7, and a notch at the gutter's edge.
    assert_eq!(
        drawn(rgba(255, 0, 0, 0x40)),
        vec![[bx, by + 139.5, 600., 1.]]
    );
    assert_eq!(
        drawn(rgba(255, 0, 0, 255)),
        vec![[bx + 58., by + 138., 6., 4.]]
    );
    assert!(drawn(rgba(0x12, 0x34, 0x56, 255)).is_empty());
    assert!(drawn(rgba(0x65, 0x43, 0x21, 255)).is_empty());

    // Fills go under the selection; a reversed range or too many is invalid.
    doc.selection = Some(CodeSelection {
        anchor: at(1, 0),
        head: at(1, 2),
    });
    m.rebuild(&mut doc);
    m.settle(&mut doc);
    m.paint();
    let rects = m.painter.rects.take();
    let fill = rects
        .iter()
        .position(|(_, c)| *c == rgba(0, 255, 0, 255))
        .unwrap();
    let selection = rects
        .iter()
        .position(|(r, _)| *r == [text_x, by + 20., 2. * col, 20.])
        .unwrap();
    assert!(fill < selection);
    let reversed = CodeDecoration {
        start: at(2, 0),
        end: at(1, 0),
        ..doc.decorations[0].clone()
    };
    doc.decorations = vec![reversed];
    assert!(view(&doc).build().is_err());
    doc.decorations = vec![doc.decorations[0].clone(); 10_001];
    doc.decorations[0].start = at(0, 0);
    assert!(view(&doc).build().is_err());
}

#[test]
fn italic_spans_indent_guides_and_scrollbar_marks() {
    let mut doc = Doc::new(200);
    doc.lines[1] = "    if x {".into();
    doc.lines[2] = String::new();
    doc.lines[3] = "        y();".into();
    doc.lines[4] = "    }".into();
    doc.guides = Some(4);
    doc.italic = true;
    doc.marks = vec![(100, "#FF0000".into()), (150, "#00FF00|#0000FF".into())];
    let mut m = Mounted::new(&mut doc);
    m.painter.rects.borrow_mut().clear();
    m.paint();
    let rects = m.painter.rects.take();
    let [bx, by, bw, bh] = m.bounds();
    // Palette 0 is italic.
    assert!(m
        .painter
        .italics
        .iter()
        .any(|(t, i)| t == "line 0 = value;" && i == &[true]));
    // One guide per 4 columns of indentation; the blank line takes the
    // depth of the line after it. Guides are 1pt wide, a line tall.
    let text_x = bx + 76.;
    let guides: Vec<[f32; 4]> = rects
        .iter()
        .map(|(r, _)| *r)
        .filter(|r| r[2] == 1. && r[3] == 20. && r[0] >= text_x && r[1] < by + 100.)
        .collect();
    let at = |line: f32, column: f32| [text_x + column * 6.5, by + line * 20., 1., 20.];
    for g in [
        at(1., 0.),
        at(2., 0.),
        at(2., 4.),
        at(3., 0.),
        at(3., 4.),
        at(4., 0.),
    ] {
        assert!(guides.contains(&g), "{g:?} in {guides:?}");
    }
    assert_eq!(guides.len(), 6);
    // Marks sit along the track by line, in their colours (dark half).
    let track = bh - 12.;
    let red = rects
        .iter()
        .find(|(_, c)| *c == [1., 0., 0., 1.])
        .unwrap()
        .0;
    assert_eq!(red, [bx + bw - 11., by + 6. + track * 0.5, 7., 2.]);
    assert!(rects.iter().any(|(_, c)| *c == [0., 0., 1., 1.]));
    assert!(!rects.iter().any(|(_, c)| *c == [0., 1., 0., 1.]));
    // Out of range is invalid.
    doc.guides = Some(0);
    assert!(view(&doc).build().is_err());
}
