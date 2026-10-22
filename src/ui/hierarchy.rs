//! Bounded, interactive hierarchy charts. All drawing/input uses ordinary Volt
//! elements. Values are inclusive subtree totals, never summed with descendants.
use super::*;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};
/// One preorder hierarchy node. Parents must precede their children.
#[derive(Clone, Debug)]
pub struct HierarchyNode {
    pub key: String,
    pub label: String,
    /// Inclusive weight; direct children's sum must not exceed this value.
    pub value: u64,
    pub parent: Option<usize>,
}
impl HierarchyNode {
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        value: u64,
        parent: Option<usize>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            value,
            parent,
        }
    }
}
/// One proportional tile in logical chart coordinates (before visual gutters).
#[derive(Clone, Copy, Debug)]
pub struct HierarchyRect {
    pub node: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
fn validate(nodes: &[HierarchyNode], width: f32, height: f32) -> Result<Vec<Vec<usize>>, String> {
    if !width.is_finite()
        || !height.is_finite()
        || !(1. ..=8192.).contains(&width)
        || !(1. ..=8192.).contains(&height)
    {
        return Err("hierarchy dimensions must be finite and in 1..8192".into());
    }
    if nodes.is_empty() || nodes.len() > 4096 {
        return Err("hierarchy requires 1..4096 nodes".into());
    }
    let mut children = vec![vec![]; nodes.len()];
    let mut keys = BTreeSet::new();
    let mut depths = vec![0; nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        if node.key.is_empty()
            || node.key.len() > 128
            || node.label.len() > 2048
            || !keys.insert(&node.key)
        {
            return Err("hierarchy keys must be nonempty, unique and bounded".into());
        }
        match node.parent {
            None if i == 0 => {}
            Some(p) if p < i => {
                depths[i] = depths[p] + 1;
                if depths[i] > 64 {
                    return Err("hierarchy depth exceeds 64".into());
                }
                children[p].push(i);
            }
            _ => return Err("hierarchy needs one first root and earlier parents".into()),
        }
    }
    for (i, child) in children.iter().enumerate() {
        if child.iter().map(|j| nodes[*j].value as u128).sum::<u128>() > nodes[i].value as u128 {
            return Err("child weights exceed inclusive parent total".into());
        }
    }
    Ok(children)
}
/// Lay out an icicle with fixed-height depth bands. Zero-weight nodes have no tile.
/// `levels` is bounded to 1..16. Values not represented by children stay blank
/// in the next band; the chart never inflates a child to consume that space.
pub fn icicle_layout(
    nodes: &[HierarchyNode],
    width: f32,
    height: f32,
    levels: usize,
) -> Result<Vec<HierarchyRect>, String> {
    icicle_layout_with_root(nodes, width, height, levels, false)
}
fn icicle_layout_with_root(
    nodes: &[HierarchyNode],
    width: f32,
    height: f32,
    levels: usize,
    hide_root: bool,
) -> Result<Vec<HierarchyRect>, String> {
    let children = validate(nodes, width, height)?;
    Ok(icicle_rects(
        nodes, width, height, levels, hide_root, &children,
    ))
}
fn icicle_rects(
    nodes: &[HierarchyNode],
    width: f32,
    height: f32,
    levels: usize,
    hide_root: bool,
    children: &[Vec<usize>],
) -> Vec<HierarchyRect> {
    let levels = levels.clamp(1, 16);
    let band = height / levels as f32;
    let hidden = usize::from(hide_root);
    let mut out = vec![];
    let mut stack = vec![(0, 0., width, 0)];
    while let Some((i, x, w, depth)) = stack.pop() {
        if nodes[i].value == 0 || w <= 0. {
            continue;
        }
        if depth >= hidden {
            out.push(HierarchyRect {
                node: i,
                x,
                y: (depth - hidden) as f32 * band,
                width: w,
                height: band,
            });
        }
        if depth + 1 < levels + hidden {
            let mut left = x;
            let mut next = vec![];
            for &child in &children[i] {
                let cw = (w as f64 * nodes[child].value as f64 / nodes[i].value as f64) as f32;
                next.push((child, left, cw, depth + 1));
                left += cw;
            }
            stack.extend(next.into_iter().rev());
        }
    }
    out
}

fn worst(row: &[(usize, f64)], side: f64) -> f64 {
    let sum = row.iter().map(|v| v.1).sum::<f64>();
    if sum <= 0. || side <= 0. {
        return f64::INFINITY;
    }
    let min = row.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
    let max = row.iter().map(|v| v.1).fold(0., f64::max);
    (side * side * max / (sum * sum)).max(sum * sum / (side * side * min))
}
/// Squarified treemap of the root's immediate children. Select a tile to drill
/// down by providing that subtree as a new root. Unrepresented bytes stay blank.
pub fn treemap_layout(
    nodes: &[HierarchyNode],
    width: f32,
    height: f32,
) -> Result<Vec<HierarchyRect>, String> {
    let children = validate(nodes, width, height)?;
    Ok(treemap_rects(nodes, width, height, &children))
}
fn treemap_rects(
    nodes: &[HierarchyNode],
    width: f32,
    height: f32,
    children: &[Vec<usize>],
) -> Vec<HierarchyRect> {
    if nodes[0].value == 0 {
        return vec![];
    }
    let total = nodes[0].value as f64;
    let scale = width as f64 * height as f64 / total;
    let mut items: Vec<_> = children[0]
        .iter()
        .filter(|i| nodes[**i].value > 0)
        .map(|i| (*i, nodes[*i].value as f64 * scale))
        .collect();
    items.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    if items.is_empty() {
        return vec![HierarchyRect {
            node: 0,
            x: 0.,
            y: 0.,
            width,
            height,
        }];
    }
    // Include a non-rendered remainder so missing children are not magnified.
    let used = items.iter().map(|v| v.1).sum::<f64>();
    let rest = (width as f64 * height as f64 - used).max(0.);
    if rest > 0.000001 {
        items.push((usize::MAX, rest));
    }
    let (mut x, mut y, mut w, mut h) = (0_f64, 0_f64, width as f64, height as f64);
    let mut out = vec![];
    let mut pos = 0;
    while pos < items.len() && w > 0. && h > 0. {
        let side = w.min(h);
        let start = pos;
        pos += 1;
        while pos < items.len()
            && worst(&items[start..=pos], side) <= worst(&items[start..pos], side)
        {
            pos += 1;
        }
        let row = &items[start..pos];
        let area = row.iter().map(|v| v.1).sum::<f64>();
        let vertical = w >= h;
        let thick = (area / side).min(if vertical { w } else { h });
        let mut offset = 0.;
        for &(node, a) in row {
            let length = a / thick;
            let r = if vertical {
                HierarchyRect {
                    node,
                    x: x as f32,
                    y: (y + offset) as f32,
                    width: thick as f32,
                    height: length as f32,
                }
            } else {
                HierarchyRect {
                    node,
                    x: (x + offset) as f32,
                    y: y as f32,
                    width: length as f32,
                    height: thick as f32,
                }
            };
            if node != usize::MAX {
                out.push(r);
            }
            offset += length;
        }
        if vertical {
            x += thick;
            w = (w - thick).max(0.);
        } else {
            y += thick;
            h = (h - thick).max(0.);
        }
    }
    out
}
type Selection<C> = Rc<RefCell<dyn FnMut(&mut C, String)>>;
type ContextOpen<C> = Rc<RefCell<dyn FnMut(&mut C, String, [f32; 2])>>;
/// Shared hierarchy chart builder. Size is logical pixels; recompute via
/// `.on_width` on a parent when the available surface changes.
pub struct HierarchyChart<C> {
    nodes: Vec<HierarchyNode>,
    size: [f32; 2],
    levels: usize,
    treemap: bool,
    key: String,
    selected: BTreeSet<String>,
    multiselectable: bool,
    reviewed: BTreeSet<String>,
    context_target: Option<String>,
    colors: BTreeMap<String, String>,
    hide_root: bool,
    keyboard: bool,
    hover_linked: bool,
    context: Option<ContextOpen<C>>,
    validation_error: Option<String>,
    pick: Option<Selection<C>>,
    format: Rc<dyn Fn(u64) -> String>,
}
pub fn icicle_chart<C>(nodes: impl IntoIterator<Item = HierarchyNode>) -> HierarchyChart<C> {
    HierarchyChart {
        nodes: nodes.into_iter().take(4097).collect(),
        size: [640., 240.],
        levels: 4,
        treemap: false,
        key: "hierarchy".into(),
        selected: BTreeSet::new(),
        multiselectable: false,
        reviewed: BTreeSet::new(),
        context_target: None,
        colors: BTreeMap::new(),
        hide_root: false,
        keyboard: false,
        hover_linked: false,
        context: None,
        validation_error: None,
        pick: None,
        format: Rc::new(|v| v.to_string()),
    }
}
pub fn treemap_chart<C>(nodes: impl IntoIterator<Item = HierarchyNode>) -> HierarchyChart<C> {
    let mut c = icicle_chart(nodes);
    c.treemap = true;
    c
}
impl<C> HierarchyChart<C> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = [width, height];
        self
    }
    pub fn levels(mut self, levels: usize) -> Self {
        self.levels = levels;
        self
    }
    pub fn selected(mut self, key: impl Into<String>) -> Self {
        self.multiselectable = false;
        self.selected.clear();
        self.selected.insert(key.into());
        self
    }
    /// Replace selection with these stable node keys. Unknown/nonpainted keys
    /// are harmless, as with the legacy single `.selected(...)` setter.
    pub fn selected_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.multiselectable = true;
        self.selected = bounded_keys(keys, &mut self.validation_error);
        self
    }
    /// A distinct passive review diamond and accessible "In review" annotation;
    /// this is never a checkbox and never changes selection or activation.
    pub fn reviewed_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        self.reviewed = bounded_keys(keys, &mut self.validation_error);
        self
    }
    /// Show the node whose application-owned actions menu is open. This is a
    /// dashed outline, distinct from selection/review, and never changes focus.
    pub fn context_target(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        if key.is_empty() || key.len() > 128 {
            self.validation_error = Some("context target key must be bounded and nonempty".into());
        }
        self.context_target = Some(key);
        self
    }
    /// Exact-node fill overrides. Other nodes retain the original branch palette.
    /// Invalid colors fail build; repeated keys use the final value. Literal
    /// fills choose a contrasting black/white foreground automatically.
    pub fn node_colors(mut self, colors: impl IntoIterator<Item = (String, String)>) -> Self {
        self.colors.clear();
        for (index, (key, color)) in colors.into_iter().take(4097).enumerate() {
            if index >= 4096
                || key.is_empty()
                || key.len() > 128
                || (crate::declarative::document::parse_color(&color).is_none()
                    && ColorToken::parse(&color).is_none())
            {
                self.validation_error =
                    Some("node_colors requires at most 4096 bounded keys and valid colors".into());
                break;
            }
            self.colors.insert(key, color);
        }
        self
    }
    /// Icicle only: omit the root band. `levels` then counts visible CHILD
    /// bands, filling the available height without inflating their proportions.
    /// Treemaps keep their existing layout, including a leaf-root fallback.
    pub fn hide_root(mut self, hide: bool) -> Self {
        self.hide_root = hide;
        self
    }
    /// Opt into one roving Tab stop. Left/Right visit visible siblings, Up the
    /// parent, Down its first visible child; Home/End visit sibling endpoints.
    /// Arrows change real keyboard focus only, Enter keeps on_select semantics.
    /// A 4px internal gutter keeps focus rings inside the chart clip (minimum
    /// chart dimensions for this mode are 12x12 logical pixels).
    pub fn keyboard_navigation(mut self, enabled: bool) -> Self {
        self.keyboard = enabled;
        self
    }
    /// Opt into paint-only hover links keyed by each node's stable key.
    /// Pair with `.hover_link(node_key)` on another component. No callbacks,
    /// navigation, selection, tooltip opening or file I/O occurs on peers.
    pub fn hover_linked(mut self, enabled: bool) -> Self {
        self.hover_linked = enabled;
        self
    }
    /// Request an application-owned node menu without selecting it. Coordinates
    /// are WINDOW-LOGICAL pixels: pointer location, or focused tile center for
    /// Shift-F10 / the Context Menu key. Escape remains owned by the app's menu.
    pub fn on_context_open(mut self, f: impl FnMut(&mut C, String, [f32; 2]) + 'static) -> Self {
        self.context = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn value_format(mut self, format: impl Fn(u64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }
    pub fn on_select(mut self, pick: impl FnMut(&mut C, String) + 'static) -> Self {
        self.pick = Some(Rc::new(RefCell::new(pick)));
        self
    }
}
fn bounded_keys(
    keys: impl IntoIterator<Item = String>,
    error: &mut Option<String>,
) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for (index, key) in keys.into_iter().take(4097).enumerate() {
        if index >= 4096 || key.is_empty() || key.len() > 128 {
            *error = Some("hierarchy state requires at most 4096 nonempty bounded keys".into());
            break;
        }
        result.insert(key);
    }
    result
}
fn invalid_chart<C>(message: impl Into<String>) -> Element<C> {
    let mut element: Element<C> = text("Invalid hierarchy chart").into();
    element.validation_error = Some(message.into());
    element
}
fn foreground(color: &str) -> &'static str {
    let Some([r, g, b, _]) = crate::declarative::document::parse_color(color) else {
        return "#FFFFFF";
    };
    let linear = |c: f32| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    if 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b) > 0.179 {
        "#111111"
    } else {
        "#FFFFFF"
    }
}
fn reviewed_tile<C>(
    tile: Element<C>,
    chart: &str,
    index: usize,
    width: f32,
    height: f32,
    ink: &'static str,
) -> Element<C> {
    let size = 8_f32.min((width - 4.).max(0.)).min((height - 4.).max(0.));
    if size < 2. {
        return tile;
    }
    let marker = canvas(|canvas| {
        canvas.fill(
            VectorPath::new()
                .move_to(size / 2., 0.)
                .line_to(size, size / 2.)
                .line_to(size / 2., size)
                .line_to(0., size / 2.)
                .close(),
            ink,
        );
    })
    .key(format!("{chart}-review-{index}"))
    .width(size)
    .height(size)
    .scroll_passthrough(true);
    zstack(vec![
        tile,
        vstack![marker]
            .width(width)
            .height(height)
            .padding(3., [Edge::Top, Edge::Right])
            .flow(Flow::TopRight)
            .scroll_passthrough(true)
            .into(),
    ])
    .width(width)
    .height(height)
    .flow(Flow::TopLeft)
    .scroll_passthrough(true)
    .into()
}
impl<C: 'static> From<HierarchyChart<C>> for Element<C> {
    fn from(c: HierarchyChart<C>) -> Self {
        use crate::declarative::{document::RovingNavigation, AccessRole};
        if let Some(error) = c.validation_error {
            return invalid_chart(error);
        }
        if c.keyboard && (c.size[0] < 12. || c.size[1] < 12.) {
            return invalid_chart("keyboard hierarchy requires at least 12x12 logical pixels");
        }
        // Validate original dimensions even when the optional focus gutter is used.
        let children = match validate(&c.nodes, c.size[0], c.size[1]) {
            Ok(v) => v,
            Err(e) => return invalid_chart(e),
        };
        let inset = if c.keyboard { 4. } else { 0. };
        let width = c.size[0] - inset * 2.;
        let height = c.size[1] - inset * 2.;
        let layout = if c.treemap {
            treemap_rects(&c.nodes, width, height, &children)
        } else {
            icicle_rects(&c.nodes, width, height, c.levels, c.hide_root, &children)
        };
        let rects: Vec<_> = layout
            .into_iter()
            .filter(|r| r.width >= 3. && r.height >= 3.)
            .collect();
        let visible: BTreeSet<_> = if c.keyboard {
            rects.iter().map(|r| r.node).collect()
        } else {
            BTreeSet::new()
        };
        // Build sibling positions once, not once per tile: wide directories
        // must not pay quadratic work just to expose arrow-key navigation.
        let visible_children: Vec<Vec<usize>> = if c.keyboard {
            children
                .iter()
                .map(|list| {
                    list.iter()
                        .copied()
                        .filter(|i| visible.contains(i))
                        .collect()
                })
                .collect()
        } else {
            vec![]
        };
        let mut positions = vec![0; if c.keyboard { c.nodes.len() } else { 0 }];
        for siblings in &visible_children {
            for (position, &index) in siblings.iter().enumerate() {
                positions[index] = position;
            }
        }
        let entry = rects
            .iter()
            .find(|r| c.selected.contains(&c.nodes[r.node].key))
            .or_else(|| rects.first())
            .map(|r| r.node);
        let tile_id = |index| format!("{}-tile-{index}", c.key);
        let group = format!("{}:{}{}", c.key.len(), c.key, c.nodes[0].key);
        let colors = [
            "#355F8A", "#326F70", "#77558C", "#856436", "#83516A", "#486A91", "#536F4B",
        ];
        let mut layers = vec![];
        for r in rects {
            let node = &c.nodes[r.node];
            let reviewed = c.reviewed.contains(&node.key);
            let selected = c.selected.contains(&node.key);
            let context_target = c.context_target.as_ref() == Some(&node.key);
            let value = (c.format)(node.value);
            let label = format!(
                "{} — {}{}",
                node.label,
                value,
                if reviewed { " — In review" } else { "" }
            );
            let budget = ((r.width - if reviewed { 30. } else { 18. }) / 7.).max(1.) as usize;
            let mut short: String = node.label.chars().take(budget).collect();
            if node.label.chars().count() > budget {
                short.push('…');
            }
            let text = if r.width > 130. && r.height > 48. {
                format!("{short}\n{value}")
            } else if r.width > 72. {
                short
            } else {
                String::new()
            };
            let mut branch = r.node;
            while c.nodes[branch].parent.is_some_and(|parent| parent != 0) {
                branch = c.nodes[branch].parent.unwrap();
            }
            let original = colors[c.nodes[branch]
                .key
                .bytes()
                .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize))
                % colors.len()];
            let override_fill = c.colors.get(&node.key);
            let fill = override_fill.map(String::as_str).unwrap_or(original);
            let ink = if override_fill.is_some() {
                foreground(fill)
            } else {
                "#FFFFFF"
            };
            let tile_w = (r.width - 2.).max(1.);
            let tile_h = (r.height - 2.).max(1.);
            let mut button = button(text)
                .key(tile_id(r.node))
                .accessible_label(&label)
                .width(tile_w)
                .height(tile_h)
                .font_size(12.)
                .font_weight(500)
                .color(ink)
                .background(fill)
                .rounded(3.)
                .border_width(if selected || context_target { 2. } else { 0. })
                .border_style(if context_target {
                    BorderStyle::Dashed
                } else {
                    BorderStyle::Solid
                })
                .border_color(ink);
            if c.hover_linked {
                // A fill change leaves selection/focus borders and the dashed
                // context target untouched; default chart appearance is unchanged.
                button = button.hover_link(&node.key).hover_style(|style| {
                    style.background(
                        crate::declarative::document::parse_color(fill)
                            .map(|rgba| {
                                let toward = if ink == "#FFFFFF" { 255. } else { 0. };
                                let channel =
                                    |v: f32| (v * 255. * 0.88 + toward * 0.12).round() as u8;
                                format!(
                                    "#{:02X}{:02X}{:02X}{:02X}",
                                    channel(rgba[0]),
                                    channel(rgba[1]),
                                    channel(rgba[2]),
                                    (rgba[3] * 255.).round() as u8
                                )
                            })
                            .unwrap_or_else(|| fill.to_string()),
                    )
                });
            }
            // Custom-palette presentation is opt-in: avoid accidental extra
            // wrapped lines on narrow tiles while preserving explicit value
            // line breaks and the legacy default chart's text metrics.
            if override_fill.is_some() {
                button = button.no_wrap(true);
            }
            button.0.node.access_selected = Some(selected);
            if c.keyboard {
                let siblings = node
                    .parent
                    .map(|p| visible_children[p].as_slice())
                    .unwrap_or_else(|| std::slice::from_ref(&r.node));
                let position = positions[r.node];
                button = button
                    .tab_stop(entry == Some(r.node))
                    .focus_style(|style| style.border_width(2.).border_color(ink));
                button.0.node.access_role = Some(AccessRole::TreeItem);
                button.0.node.access_parent = Some(
                    node.parent
                        .filter(|i| visible.contains(i))
                        .map(tile_id)
                        .unwrap_or_else(|| c.key.clone()),
                );
                button.0.node.access_expanded =
                    (!visible_children[r.node].is_empty()).then_some(true);
                button.0.node.access_position = Some(position);
                button.0.node.access_set_size = Some(siblings.len());
                let mut depth = 0_usize;
                let mut parent = node.parent;
                while let Some(p) = parent {
                    depth += 1;
                    parent = c.nodes[p].parent;
                }
                button.0.node.access_level =
                    Some(depth.saturating_sub(usize::from(c.hide_root && !c.treemap)));
                button.0.node.roving = Some(RovingNavigation {
                    group: group.clone(),
                    member: node.key.clone(),
                    left: position.checked_sub(1).map(|i| tile_id(siblings[i])),
                    right: siblings.get(position + 1).copied().map(tile_id),
                    parent: node.parent.filter(|i| visible.contains(i)).map(tile_id),
                    child: visible_children[r.node].first().copied().map(tile_id),
                    first: siblings.first().copied().map(tile_id),
                    last: siblings.last().copied().map(tile_id),
                });
            }
            if let Some(pick) = &c.pick {
                let pick = pick.clone();
                let key = node.key.clone();
                button = button.on_click(move |app| (pick.borrow_mut())(app, key.clone()));
            }
            let mut tile: Element<C> = tooltip(&label, button)
                .key(format!("{}-tip-{}", c.key, r.node))
                .into();
            if reviewed {
                tile = reviewed_tile(tile, &c.key, r.node, tile_w, tile_h, ink);
            }
            if let Some(open) = &c.context {
                let open = open.clone();
                let key = node.key.clone();
                let mut context = zstack(vec![tile])
                    .key(format!("{}-context-{}", c.key, r.node))
                    .width(tile_w)
                    .height(tile_h)
                    .flow(Flow::TopLeft);
                context.0.callback = Some(Callback::Context(Box::new(move |app, position| {
                    (open.borrow_mut())(app, key.clone(), position)
                })));
                tile = context.into();
            }
            if c.hover_linked {
                tile.node.hover_link = Some(node.key.clone());
            }
            layers.push(
                vstack![tile]
                    .padding(r.x + inset, Edge::Left)
                    .padding(r.y + inset, Edge::Top)
                    .width(c.size[0])
                    .height(c.size[1])
                    .flow(Flow::TopLeft)
                    .scroll_passthrough(true)
                    .into(),
            );
        }
        let mut chart = zstack(layers)
            .key(c.key)
            .width(c.size[0])
            .height(c.size[1])
            .flow(Flow::TopLeft)
            .clip_contents(true);
        chart.0.node.access_multiselectable = c.multiselectable;
        if c.keyboard {
            chart.0.node.access_role = Some(AccessRole::Tree);
        }
        chart.into()
    }
}
