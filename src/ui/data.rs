//! Bounded data composition. Selection and expansion remain application-owned.
use super::*;
use crate::declarative::{document::VirtualListProps, AccessTableInfo};
use std::{cell::RefCell, cmp::Ordering, ops::Range, rc::Rc};
type SelectFn<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
type WindowFn<C> = Box<dyn FnMut(&mut C, Range<usize>)>;

pub struct DataItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
}
pub fn data_item<T>(value: T, label: impl Into<String>) -> DataItem<T> {
    DataItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
    }
}
impl<T> DataItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
// These eager composites compute internal row widths while lowering. Fixed
// spacing setters therefore update the scalar sides used by that calculation;
// responsive layouts belong to their containing stack instead.
macro_rules! fixed_panel_spacing {
    ($target:ident, [$($generic:ident),*]) => {
        impl<C, $($generic),*> $target<C, $($generic),*> {
            /// Fixed logical content padding; last write to an edge wins.
            pub fn padding(mut self, value: f32, edges: impl Into<Edges>) -> Self {
                let mask = edges.into().0;
                for i in (0..4).filter(|i| mask & (1 << i) != 0) {
                    let side = match i { 0=>&mut self.0.node.padding_top, 1=>&mut self.0.node.padding_right, 2=>&mut self.0.node.padding_bottom, _=>&mut self.0.node.padding_left };
                    *side = Some(value);
                    self.0.node.responsive.padding[i] = None;
                }
                self
            }
            /// Fixed spacing between immediate panel children. Table separators
            /// are children too; use row_height for ordinary table density.
            pub fn gap(mut self, value: f32, axes: impl Into<Axes>) -> Self {
                let mask = axes.into().0;
                for i in (0..2).filter(|i| mask & (1 << i) != 0) {
                    if i == 0 { self.0.node.gap_horizontal = Some(value); }
                    else { self.0.node.gap_vertical = Some(value); }
                    self.0.node.responsive.gap[i] = None;
                }
                self
            }
        }
    };
}
fixed_panel_spacing!(Table, [T, K]);
fixed_panel_spacing!(TreeView, [T]);
/// Only the requested fixed-height window calls the row factory. It may borrow
/// view-local data; selected IDs outside this window are intentionally allowed.
pub struct VirtualList<C, T, F>(pub(super) Element<C>, VirtualOptions<C, T, F>);
struct VirtualOptions<C, T, F> {
    count: usize,
    row_height: f32,
    window: Option<Range<usize>>,
    overscan: usize,
    factory: F,
    selected: Option<T>,
    enabled: bool,
    on_select: Option<SelectFn<C, T>>,
    on_window: Option<WindowFn<C>>,
}
pub fn virtual_list<C, T, F: FnMut(usize) -> DataItem<T>>(
    count: usize,
    row_height: f32,
    factory: F,
) -> VirtualList<C, T, F> {
    let mut e = Element::new(Kind::Panel);
    e.node.height = Some(320.);
    e.node.width = Some(360.);
    e.node.surface.scroll = Some(Scroll::Vertical);
    e.node.access_role = Some(AccessRole::ListBox);
    VirtualList(
        e,
        VirtualOptions {
            count,
            row_height,
            window: None,
            overscan: 2,
            factory,
            selected: None,
            enabled: true,
            on_select: None,
            on_window: None,
        },
    )
}
impl<C, T, F> VirtualList<C, T, F> {
    pub fn window(mut self, value: Range<usize>) -> Self {
        self.1.window = Some(value);
        self
    }
    pub fn overscan(mut self, value: usize) -> Self {
        self.1.overscan = value;
        self
    }
    pub fn selected(mut self, value: Option<T>) -> Self {
        self.1.selected = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.1.enabled = value;
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn on_window_change(mut self, f: impl FnMut(&mut C, Range<usize>) + 'static) -> Self {
        self.1.on_window = Some(Box::new(f));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static, F: FnMut(usize) -> DataItem<T>>
    From<VirtualList<C, T, F>> for Element<C>
{
    fn from(mut list: VirtualList<C, T, F>) -> Self {
        let h = list.0.node.height.unwrap_or(320.);
        let rows = (h / list.1.row_height).ceil() as usize;
        let window = list.1.window.clone().unwrap_or(
            0..list.1.count.min(
                rows.saturating_add(list.1.overscan.saturating_mul(2))
                    .saturating_add(1),
            ),
        );
        let error = if list.0.auto_id {
            Some("virtual list requires a stable .key(...)")
        } else if !list.1.row_height.is_finite()
            || !(8.0..=512.0).contains(&list.1.row_height)
            || list.1.count > 10_000_000
            || (list.1.count as f64 * list.1.row_height as f64) > 1e9
            || list.1.overscan > 16
            || !h.is_finite()
            || h < 0.
            || rows
                .saturating_add(list.1.overscan.saturating_mul(2))
                .saturating_add(1)
                > 96
        {
            Some("virtual list viewport, row height, overscan or count exceeds bounds")
        } else if window.start > window.end || window.end > list.1.count || window.len() > 96 {
            Some("virtual list window must be ordered, in range and at most 96 rows")
        } else if list.1.on_select.is_none() || list.1.on_window.is_none() {
            Some("virtual list requires on_select and on_window_change")
        } else {
            None
        };
        if let Some(error) = error {
            list.0.validation_error = Some(error.into());
            return list.0;
        }
        let callback = list.1.on_select.take().unwrap();
        let mut values = Vec::new();
        for index in window.clone() {
            let item = (list.1.factory)(index);
            let Some(key) = item.key else {
                list.0.validation_error = Some("virtual row requires a stable item key".into());
                return list.0;
            };
            if values.iter().any(|value| value == &item.value) {
                list.0.validation_error = Some("duplicate mounted virtual list value".into());
                return list.0;
            }
            values.push(item.value.clone());
            let selected = list.1.selected.as_ref() == Some(&item.value);
            let handler = callback.clone();
            let value = item.value;
            let mut row = button(item.label)
                .key(key)
                .height(list.1.row_height)
                .enabled(list.1.enabled && item.enabled)
                .primary(selected)
                .on_click(move |app: &mut C| {
                    if !selected {
                        (handler.borrow_mut())(app, value.clone());
                    }
                });
            // A fill-width list must stretch its rows too, not keep the default 360px.
            row.0.node.width = list.0.node.width;
            if selected {
                row = row.color(ColorToken::PrimaryContent);
            }
            row.0.node.checked = selected;
            row.0.node.access_role = Some(AccessRole::ListBoxOption);
            row.0.node.access_selected = Some(selected);
            row.0.node.access_position = Some(index);
            list.0.children.push(row.into());
        }
        list.0.node.virtual_list = Some(VirtualListProps {
            count: list.1.count,
            start: window.start,
            end: window.end,
            row_height: list.1.row_height,
            overscan: list.1.overscan,
            on_window_change: None,
            estimates: None,
            generation: 0,
            scroll_to_index: None,
        });
        list.0.callback = Some(Callback::Window(list.1.on_window.take().unwrap()));
        list.0
    }
}

/// A scrolling column of variable-height rows where only the rows near the
/// viewport are built, so documents and feeds of any length stay within the
/// element budget. Rows are measured as they mount; `.estimate(..)` or
/// `.estimates(..)` stand in for rows not yet seen.
///
/// ```
/// use volt_ui_kit::ui::prelude::*;
/// struct App { window: std::ops::Range<usize> }
/// fn view(app: &App) -> Element<App> {
///     lazy_list(10_000, |i| text(format!("Paragraph {i}")).key(format!("p{i}")).into())
///         .key("doc")
///         .estimate(24.)
///         .window(app.window.clone())
///         .on_window_change(|app: &mut App, window| app.window = window)
///         .gap(8.)
///         .into()
/// }
/// view(&App { window: 0..30 }).build()?;
/// # Ok::<(), volt_ui_kit::declarative::DocumentError>(())
/// ```
pub struct LazyList<C, F>(pub(super) Element<C>, LazyOptions<C, F>);
struct LazyOptions<C, F> {
    count: usize,
    factory: F,
    estimate: f32,
    estimates: Option<Vec<f32>>,
    window: Option<Range<usize>>,
    overscan: usize,
    generation: u64,
    scroll_to: Option<(usize, u32)>,
    on_window: Option<WindowCallback<C>>,
}
const MAX_LAZY_ROWS: usize = 1_000_000;
/// See [`LazyList`]. `row(i)` builds row `i`; give each row a stable key.
pub fn lazy_list<C, F: FnMut(usize) -> Element<C>>(count: usize, row: F) -> LazyList<C, F> {
    let mut e = Element::new(Kind::Panel);
    e.node.surface.scroll = Some(Scroll::Vertical);
    e.node.style.background = Some("#00000000".into());
    e.node.style.border_width = Some(0.);
    e.node.style.radius = Some(Rounding::Radius(0.));
    LazyList(
        e,
        LazyOptions {
            count,
            factory: row,
            estimate: 48.,
            estimates: None,
            window: None,
            overscan: 3,
            generation: 0,
            scroll_to: None,
            on_window: None,
        },
    )
}
impl<C, F> LazyList<C, F> {
    /// Called with the selected text when a selection inside the list
    /// changes (see `.selectable(true)`); empty when it is cleared.
    pub fn on_select_text(mut self, f: impl FnMut(&mut C, String) + 'static) -> Self {
        self.0.select_text = Some(Box::new(f));
        self
    }
    /// One estimated height (logical pixels) for every unmeasured row.
    pub fn estimate(mut self, height: f32) -> Self {
        self.1.estimate = height;
        self
    }
    /// An estimated height per row; better estimates keep the scroll bar
    /// steadier while rows are first measured.
    pub fn estimates(mut self, heights: impl IntoIterator<Item = f32>) -> Self {
        // One extra value is sufficient to reject excess input, even an
        // infinite iterator. Invalid counts must not drive an allocation.
        if self.1.count > MAX_LAZY_ROWS {
            self.0.validation_error = Some("lazy list count exceeds bounds".into());
            return self;
        }
        let limit = self.1.count + 1;
        self.1.estimates = Some(heights.into_iter().take(limit).collect());
        self
    }
    /// The rows to build, as last reported by `on_window_change`.
    pub fn window(mut self, value: Range<usize>) -> Self {
        self.1.window = Some(value);
        self
    }
    /// Space between rows, in logical pixels.
    pub fn gap(mut self, value: f32) -> Self {
        self.0.node.gap = value;
        self
    }
    /// Extra rows kept built above and below the viewport (0–16).
    pub fn overscan(mut self, rows: usize) -> Self {
        self.1.overscan = rows;
        self
    }
    /// Change this when row contents change (a new document): measured
    /// heights are discarded and rows are measured again.
    pub fn generation(mut self, value: u64) -> Self {
        self.1.generation = value;
        self
    }
    /// Scroll so `row` is at the top whenever `request` changes (bump a
    /// counter to jump again). Works for rows that are not built yet.
    pub fn scroll_to_row(mut self, row: usize, request: u32) -> Self {
        self.1.scroll_to = Some((row, request));
        self
    }
    /// Called with the new range of rows to build as the list scrolls.
    pub fn on_window_change(mut self, f: impl FnMut(&mut C, Range<usize>) + 'static) -> Self {
        self.1.on_window = Some(Box::new(f));
        self
    }
}
impl<C: 'static, F: FnMut(usize) -> Element<C>> From<LazyList<C, F>> for Element<C> {
    fn from(mut list: LazyList<C, F>) -> Self {
        let count = list.1.count;
        let window = list.1.window.clone().unwrap_or(0..count.min(32));
        let error = if list.0.auto_id {
            Some("lazy list requires a stable .key(...)")
        } else if list.1.on_window.is_none() {
            Some("lazy list requires on_window_change")
        } else if count > MAX_LAZY_ROWS || list.1.overscan > 16 {
            Some("lazy list count, overscan or estimates exceed bounds")
        } else if window.start > window.end
            || window.end > count
            || window.len() > crate::declarative::document::MAX_LAZY_WINDOW
        {
            Some("lazy list window must be ordered, in range and at most 256 rows")
        } else if match list.1.estimates.as_ref() {
            Some(estimates) => {
                estimates.len() != count
                    || estimates
                        .iter()
                        .any(|h| !h.is_finite() || !(1. ..=100_000.).contains(h))
            }
            None => {
                count > 0
                    && (!list.1.estimate.is_finite()
                        || !(1. ..=100_000.).contains(&list.1.estimate))
            }
        } {
            Some("variable list needs one finite estimate (1–100000) per row")
        } else {
            None
        };
        if let Some(error) = error {
            list.0.validation_error = Some(error.into());
            return list.0;
        }
        let estimates = list
            .1
            .estimates
            .take()
            .unwrap_or_else(|| vec![list.1.estimate; count]);
        for index in window.clone() {
            let row = (list.1.factory)(index);
            if row.auto_id {
                list.0.validation_error = Some("lazy list rows require stable keys".into());
                return list.0;
            }
            list.0.children.push(row);
        }
        list.0.node.virtual_list = Some(VirtualListProps {
            count,
            start: window.start,
            end: window.end,
            row_height: 1.,
            overscan: list.1.overscan,
            on_window_change: None,
            estimates: Some(estimates.into()),
            generation: list.1.generation,
            scroll_to_index: list.1.scroll_to.filter(|(row, _)| *row < count),
        });
        list.0.callback = Some(Callback::Window(list.1.on_window.take().unwrap()));
        list.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}
impl SortDirection {
    fn reversed(self) -> Self {
        match self {
            Self::Ascending => Self::Descending,
            Self::Descending => Self::Ascending,
        }
    }
}
pub struct TableColumn<K> {
    value: K,
    label: String,
    width: f32,
    sortable: bool,
    align: Align,
}
pub fn table_column<K>(value: K, label: impl Into<String>) -> TableColumn<K> {
    TableColumn {
        value,
        label: label.into(),
        width: 120.,
        sortable: true,
        align: Align::Start,
    }
}
impl<K> TableColumn<K> {
    /// Align header and body values consistently (End is useful for numbers).
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    pub fn sortable(mut self, value: bool) -> Self {
        self.sortable = value;
        self
    }
}
pub struct TableRow<T> {
    value: T,
    cells: Vec<String>,
    key: Option<String>,
    enabled: bool,
}
pub fn table_row<T>(value: T, cells: impl IntoIterator<Item = impl Into<String>>) -> TableRow<T> {
    TableRow {
        value,
        cells: cells.into_iter().take(9).map(Into::into).collect(),
        key: None,
        enabled: true,
    }
}
impl<T> TableRow<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
type SortFn<C, K> = Rc<RefCell<dyn FnMut(&mut C, K, SortDirection)>>;
type Compare<T, K> = Box<dyn Fn(&K, &T, &T) -> Ordering>;
pub struct Table<C, T, K>(pub(super) Element<C>, TableOptions<C, T, K>);
struct TableOptions<C, T, K> {
    row_height: f32,
    header_height: f32,
    cell_padding: f32,
    columns: Vec<TableColumn<K>>,
    rows: Vec<TableRow<T>>,
    sort: Option<(K, SortDirection)>,
    compare: Option<Compare<T, K>>,
    selected: Option<T>,
    enabled: bool,
    on_select: Option<SelectFn<C, T>>,
    on_sort: Option<SortFn<C, K>>,
}
pub fn table<C, T, K>(
    columns: impl IntoIterator<Item = TableColumn<K>>,
    rows: impl IntoIterator<Item = TableRow<T>>,
) -> Table<C, T, K> {
    // Natural height by default; `.height(...)` makes the rows scroll.
    let mut e = Element::new(Kind::Panel);
    e.node.surface.scroll = Some(Scroll::Both);
    e.node.padding = 1.; // Reserve only the border stroke, not a visual gutter.
    e.node.style.background = Some(ColorToken::Base100.into());
    e.node.style.border_color = Some(ColorToken::Border.into());
    e.node.style.border_width = Some(1.);
    e.node.style.radius = Some(Rounding::Radius(12.));
    e.node.style.font_size = Some(13.);
    e.node.access_role = Some(AccessRole::Table);
    Table(
        e,
        TableOptions {
            row_height: 44.,
            header_height: 40.,
            cell_padding: 12.,
            columns: columns.into_iter().take(9).collect(),
            rows: rows.into_iter().take(97).collect(),
            sort: None,
            compare: None,
            selected: None,
            enabled: true,
            on_select: None,
            on_sort: None,
        },
    )
}
impl<C, T, K> Table<C, T, K> {
    /// Body row height in logical pixels (default 44, minimum 24).
    pub fn row_height(mut self, value: f32) -> Self {
        self.1.row_height = value;
        self
    }
    /// Header height in logical pixels (default 40, minimum 24).
    pub fn header_height(mut self, value: f32) -> Self {
        self.1.header_height = value;
        self
    }
    /// Horizontal inset of header/body cell labels (default 12).
    pub fn cell_padding(mut self, value: f32) -> Self {
        self.1.cell_padding = value;
        self
    }

    pub fn sort(mut self, column: K, direction: SortDirection) -> Self {
        self.1.sort = Some((column, direction));
        self
    }
    pub fn sort_by(mut self, compare: impl Fn(&K, &T, &T) -> Ordering + 'static) -> Self {
        self.1.compare = Some(Box::new(compare));
        self
    }
    pub fn selected(mut self, value: Option<T>) -> Self {
        self.1.selected = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.1.enabled = value;
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn on_sort(mut self, f: impl FnMut(&mut C, K, SortDirection) + 'static) -> Self {
        self.1.on_sort = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static, K: Clone + PartialEq + 'static>
    From<Table<C, T, K>> for Element<C>
{
    fn from(mut table: Table<C, T, K>) -> Self {
        let columns = &table.1.columns;
        let rows = &table.1.rows;
        // Report the first specific problem rather than a combined message.
        let error = if table.0.auto_id {
            Some("table requires a stable .key(...)")
        } else if columns.is_empty() || columns.len() > 8 {
            Some("a table needs 1..=8 columns")
        } else if rows.len() > 96
            || 2 + columns.len() + rows.len() * (3 + 2 * columns.len()) >= MAX_NODES
        {
            Some("a table holds at most 96 rows; use virtual_list for larger collections")
        } else if !columns.iter().enumerate().all(|(i, c)| {
            c.width.is_finite()
                && (32.0..=600.0).contains(&c.width)
                && !columns[..i].iter().any(|o| o.value == c.value)
        }) {
            Some("column widths must be 32..=600 and column values unique")
        } else if !rows.iter().all(|r| r.cells.len() == columns.len()) {
            Some("every row needs exactly one cell per column")
        } else if !rows
            .iter()
            .enumerate()
            .all(|(i, r)| r.key.is_some() && !rows[..i].iter().any(|o| o.value == r.value))
        {
            Some("every table_row needs a .key(...) and a unique value")
        } else if !table
            .1
            .selected
            .as_ref()
            .is_none_or(|value| rows.iter().any(|row| &row.value == value))
        {
            Some("selected(...) is not a row in the table")
        } else if !table.1.sort.as_ref().is_none_or(|(key, _)| {
            columns
                .iter()
                .any(|column| &column.value == key && column.sortable)
        }) {
            Some("sort(...) names a column that is missing or not sortable")
        } else if table.1.on_select.is_none() {
            Some("table requires on_select")
        } else if columns.iter().any(|column| column.sortable) && table.1.on_sort.is_none() {
            Some("columns are sortable by default: add on_sort or mark them .sortable(false)")
        } else {
            None
        };
        if let Some(error) = error {
            table.0.validation_error = Some(error.into());
            return table.0;
        }
        if !table.1.row_height.is_finite()
            || !(24. ..=512.).contains(&table.1.row_height)
            || !table.1.header_height.is_finite()
            || !(24. ..=512.).contains(&table.1.header_height)
            || !table.1.cell_padding.is_finite()
            || !(0. ..=256.).contains(&table.1.cell_padding)
        {
            table.0.validation_error =
                Some("table row/header height must be 24..512 and cell padding 0..256".into());
            return table.0;
        }
        let minimum = columns.iter().map(|c| c.width).sum::<f32>();
        let [top, right, bottom, left] = table.0.node.padding_sides();
        let width =
            (table.0.node.width.unwrap_or(minimum + left + right) - left - right).max(minimum);
        table.0.node.width = table.0.node.width.or(Some(width + left + right));
        for column in &mut table.1.columns {
            column.width *= width / minimum;
        }
        table.0.node.access_table = Some(AccessTableInfo {
            rows: Some(table.1.rows.len() + 1), // includes the header row
            columns: Some(table.1.columns.len()),
            ..AccessTableInfo::default()
        });
        if let Some((key, direction)) = &table.1.sort {
            let index = table
                .1
                .columns
                .iter()
                .position(|column| &column.value == key)
                .unwrap();
            let compare = table.1.compare.as_ref();
            table.1.rows.sort_by(|a, b| {
                let result = compare.map_or_else(
                    || a.cells[index].cmp(&b.cells[index]),
                    |compare| compare(key, &a.value, &b.value),
                );
                if *direction == SortDirection::Descending {
                    result.reverse()
                } else {
                    result
                }
            });
        }
        // Preserve one shared silhouette instead of independent rounded pills.
        // Explicit corner rules also select the painter's continuous inset outline,
        // rather than the compatibility uniform-radius line-segment border.
        let outer = table
            .0
            .node
            .style
            .corners
            .unwrap_or([table.0.node.style.radius.unwrap_or(Rounding::Radius(12.)); 4]);
        table.0.node.style.corners = Some(outer);
        let natural_height = top
            + bottom
            + table.1.header_height
            + table.1.rows.len() as f32 * (table.1.row_height + 1.)
            + table.1.rows.len() as f32 * 2. * table.0.node.main_gap(1);
        let outer_limit = table
            .0
            .node
            .width
            .unwrap_or(width + left + right)
            .min(table.0.node.height.unwrap_or(natural_height))
            * 0.5;
        let insets = [
            top.max(left),
            top.max(right),
            bottom.max(right),
            bottom.max(left),
        ];
        let inner = std::array::from_fn::<_, 4, _>(|i| {
            let radius = match outer[i] {
                Rounding::Radius(v) => v.min(outer_limit),
                Rounding::Full => outer_limit,
            };
            Rounding::Radius((radius - insets[i]).max(0.))
        });
        let zero = Rounding::Radius(0.);
        let row_corner = if outer.iter().all(|r| *r == zero) {
            zero
        } else {
            Rounding::Radius(4.)
        };
        let mut header_corners = [inner[0], inner[1], row_corner, row_corner];
        if table.1.rows.is_empty() {
            header_corners[2] = inner[2];
            header_corners[3] = inner[3];
        }
        let mut header = Element::new(Kind::Row);
        header.node.height = Some(table.1.header_height);
        header.node.width = Some(width);
        header.node.style.background = Some(ColorToken::Base200.into());
        header.node.style.corners = Some(header_corners);
        header.node.access_role = Some(AccessRole::Row);
        header.node.access_table = Some(AccessTableInfo {
            row: Some(0),
            ..AccessTableInfo::default()
        });
        let columns = table.1.columns;
        for (i, column) in columns.iter().enumerate() {
            let sorted = table
                .1
                .sort
                .as_ref()
                .filter(|(key, _)| key == &column.value);
            let direction = sorted.map_or(SortDirection::Ascending, |(_, d)| d.reversed());
            let callback = table.1.on_sort.clone();
            let key = column.value.clone();
            let mut cell: Element<C> = if column.sortable {
                button("")
                    .key(format!("{}-column-{i}", table.0.node.id))
                    .variant(ButtonVariant::Ghost)
                    .rounded(0.)
                    .enabled(table.1.enabled)
                    .on_click(move |app: &mut C| {
                        if let Some(callback) = &callback {
                            (callback.borrow_mut())(app, key.clone(), direction);
                        }
                    })
                    .into()
            } else {
                Element::new(Kind::Row)
            };
            cell.node.style.corners = Some([
                if i == 0 {
                    header_corners[0]
                } else {
                    row_corner
                },
                if i + 1 == columns.len() {
                    header_corners[1]
                } else {
                    row_corner
                },
                if i + 1 == columns.len() {
                    header_corners[2]
                } else {
                    row_corner
                },
                if i == 0 {
                    header_corners[3]
                } else {
                    row_corner
                },
            ]);
            cell.node.width = Some(column.width);
            cell.node.height = Some(table.1.header_height);
            let mut contents = Element::new(Kind::Row);
            contents.node.width = Some(column.width);
            contents.node.height = Some(table.1.header_height);
            contents.node.padding = table.1.cell_padding;
            contents.node.padding_top = Some(0.);
            contents.node.padding_bottom = Some(0.);
            contents.node.align = Align::Center;
            contents.node.flow = Some(cell_flow(column.align));
            contents.node.gap = 6.;
            cell.node.access_role = Some(AccessRole::ColumnHeader);
            cell.node.accessible_label = Some(bounded_semantic_name(&column.label, "Column"));
            cell.node.access_table = Some(AccessTableInfo {
                row: Some(0),
                column: Some(i),
                sort_ascending: sorted.map(|(_, d)| *d == SortDirection::Ascending),
                ..AccessTableInfo::default()
            });
            contents.children.push(
                text(&column.label)
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::Muted)
                    .into(),
            );
            if let Some((_, direction)) = sorted {
                contents
                    .children
                    .push(data_sort_chevron(*direction == SortDirection::Descending));
            }
            cell.children.push(contents);
            header.children.push(cell);
        }
        table.0.children.push(header);
        if !table.1.rows.is_empty() {
            table.0.children.push(data_rule(width));
        }
        let on_select = table.1.on_select.unwrap();
        let count = table.1.rows.len();
        for (row_index, row) in table.1.rows.into_iter().enumerate() {
            let active = table.1.selected.as_ref() == Some(&row.value);
            let handler = on_select.clone();
            let value = row.value;
            let enabled = table.1.enabled && row.enabled;
            // One real row control owns its cells: no competing overlay hit layer.
            let mut hit: Element<C> = button("")
                .accessible_label(bounded_semantic_name(&row.cells.join(", "), "Table row"))
                .key(row.key.unwrap())
                .variant(if active {
                    ButtonVariant::Soft
                } else {
                    ButtonVariant::Ghost
                })
                .width(width)
                .height(table.1.row_height)
                .rounded(0.)
                .enabled(enabled)
                .opacity(if enabled { 1. } else { 0.45 })
                .on_click(move |app: &mut C| {
                    if !active {
                        (handler.borrow_mut())(app, value.clone());
                    }
                })
                .into();
            hit.node.style.corners = Some([
                row_corner,
                row_corner,
                if row_index + 1 == count {
                    inner[2]
                } else {
                    row_corner
                },
                if row_index + 1 == count {
                    inner[3]
                } else {
                    row_corner
                },
            ]);
            let mut contents = Element::new(Kind::Row);
            contents.node.width = Some(width);
            contents.node.height = Some(table.1.row_height);
            contents.node.align = Align::Center;
            hit.node.access_role = Some(AccessRole::Row);
            hit.node.access_selected = Some(active);
            hit.node.access_table = Some(AccessTableInfo {
                row: Some(row_index + 1),
                ..AccessTableInfo::default()
            });
            for (i, label) in row.cells.into_iter().enumerate() {
                let column = &columns[i];
                let mut cell = Element::new(Kind::Row);
                cell.node.width = Some(column.width);
                cell.node.height = Some(table.1.row_height);
                cell.node.padding = table.1.cell_padding;
                cell.node.padding_top = Some(0.);
                cell.node.padding_bottom = Some(0.);
                cell.node.flow = Some(cell_flow(column.align));
                let mut value = text(label);
                if active {
                    value = value.color(ColorToken::Primary).font_weight(500);
                }
                value.0.node.access_role = Some(AccessRole::Cell);
                value.0.node.access_table = Some(AccessTableInfo {
                    row: Some(row_index + 1),
                    column: Some(i),
                    ..AccessTableInfo::default()
                });
                cell.children.push(value.into());
                contents.children.push(cell);
            }
            hit.children.push(contents);
            table.0.children.push(hit);
            if row_index + 1 < count {
                table.0.children.push(data_rule(width));
            }
        }
        table.0
    }
}

pub struct TreeNode<T> {
    item: DataItem<T>,
    children: Vec<TreeNode<T>>,
}
pub fn tree_node<T>(value: T, label: impl Into<String>) -> TreeNode<T> {
    TreeNode {
        item: data_item(value, label),
        children: vec![],
    }
}
impl<T> TreeNode<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.item = self.item.key(key);
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.item = self.item.enabled(value);
        self
    }
    pub fn children(mut self, children: impl IntoIterator<Item = TreeNode<T>>) -> Self {
        self.children = children.into_iter().take(129).collect();
        self
    }
}
type ExpandFn<C, T> = Rc<RefCell<dyn FnMut(&mut C, T, bool)>>;
pub struct TreeView<C, T>(pub(super) Element<C>, TreeOptions<C, T>);
struct TreeOptions<C, T> {
    row_height: f32,
    indent_width: f32,
    roots: Vec<TreeNode<T>>,
    expanded: Vec<T>,
    selected: Option<T>,
    enabled: bool,
    on_select: Option<SelectFn<C, T>>,
    on_expand: Option<ExpandFn<C, T>>,
}
pub fn tree_view<C, T>(roots: impl IntoIterator<Item = TreeNode<T>>) -> TreeView<C, T> {
    let mut e = Element::new(Kind::Panel);
    e.node.width = Some(360.);
    e.node.surface.scroll = Some(Scroll::Vertical);
    e.node.style.font_size = Some(13.);
    e.node.access_role = Some(AccessRole::Tree);
    e.node.surface.scroll = Some(Scroll::Both);
    e.node.padding = 6.;
    e.node.gap = 2.;
    e.node.style.background = Some(ColorToken::Base100.into());
    e.node.style.border_color = Some(ColorToken::Border.into());
    e.node.style.border_width = Some(1.);
    e.node.style.radius = Some(Rounding::Radius(8.));
    TreeView(
        e,
        TreeOptions {
            row_height: 36.,
            indent_width: 18.,
            roots: roots.into_iter().take(129).collect(),
            expanded: vec![],
            selected: None,
            enabled: true,
            on_select: None,
            on_expand: None,
        },
    )
}
impl<C, T> TreeView<C, T> {
    /// Row height (default 36, minimum 28 to contain disclosure controls).
    pub fn row_height(mut self, value: f32) -> Self {
        self.1.row_height = value;
        self
    }
    /// Indentation per nesting level (default 18 logical pixels).
    pub fn indent_width(mut self, value: f32) -> Self {
        self.1.indent_width = value;
        self
    }

    pub fn expanded(mut self, values: impl IntoIterator<Item = T>) -> Self {
        self.1.expanded = values.into_iter().take(129).collect();
        self
    }
    pub fn selected(mut self, value: Option<T>) -> Self {
        self.1.selected = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.1.enabled = value;
        self
    }
    pub fn on_select(mut self, f: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.on_select = Some(Rc::new(RefCell::new(f)));
        self
    }
    pub fn on_expand(mut self, f: impl FnMut(&mut C, T, bool) + 'static) -> Self {
        self.1.on_expand = Some(Rc::new(RefCell::new(f)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<TreeView<C, T>> for Element<C> {
    fn from(mut tree: TreeView<C, T>) -> Self {
        fn values<'a, T: PartialEq>(
            nodes: &'a [TreeNode<T>],
            depth: usize,
            seen: &mut Vec<&'a T>,
        ) -> bool {
            if depth > 16 {
                return false;
            }
            for node in nodes {
                if seen.len() >= 128 || node.item.key.is_none() || seen.contains(&&node.item.value)
                {
                    return false;
                }
                seen.push(&node.item.value);
                if !values(&node.children, depth + 1, seen) {
                    return false;
                }
            }
            true
        }
        let mut seen = vec![];
        // Report the first specific problem rather than a combined message.
        let error = if tree.0.auto_id {
            Some("tree_view requires a stable .key(...)")
        } else if !values(&tree.1.roots, 0, &mut seen) {
            Some("every tree_node needs a unique value and a .key(...); trees allow at most 128 nodes, 16 levels deep")
        } else if tree.1.expanded.len() > 128
            || !tree.1.expanded.iter().all(|id| seen.contains(&id))
        {
            Some("expanded(...) names a value that is not a node in the tree")
        } else if !tree.1.selected.as_ref().is_none_or(|id| seen.contains(&id)) {
            Some("selected(...) is not a node in the tree")
        } else if tree.1.on_select.is_none() || tree.1.on_expand.is_none() {
            Some("tree_view requires on_select and on_expand")
        } else {
            None
        };
        if let Some(error) = error {
            tree.0.validation_error = Some(error.into());
            return tree.0;
        }
        if !tree.1.row_height.is_finite()
            || !(28. ..=512.).contains(&tree.1.row_height)
            || !tree.1.indent_width.is_finite()
            || !(0. ..=256.).contains(&tree.1.indent_width)
        {
            tree.0.validation_error =
                Some("tree row height must be 28..512 and indent width 0..256".into());
            return tree.0;
        }
        let on_select = tree.1.on_select.unwrap();
        let on_expand = tree.1.on_expand.unwrap();
        let [_, right, _, left] = tree.0.node.padding_sides();
        let width = (tree.0.node.width.unwrap_or(360.) - left - right).max(0.);
        tree.0.node.access_set_size = Some(tree.1.roots.len());
        struct TreeContext<'a, C, T> {
            row_height: f32,
            indent_width: f32,
            expanded: &'a [T],
            selected: Option<&'a T>,
            enabled: bool,
            width: f32,
            on_select: &'a SelectFn<C, T>,
            on_expand: &'a ExpandFn<C, T>,
        }
        fn append<C: 'static, T: Clone + PartialEq + 'static>(
            nodes: Vec<TreeNode<T>>,
            depth: usize,
            parent: Option<&str>,
            context: &TreeContext<'_, C, T>,
            out: &mut Vec<Element<C>>,
        ) -> bool {
            for (position, node) in nodes.into_iter().enumerate() {
                if out.len() >= 96 {
                    return false;
                }
                let item = node.item;
                let open = context.expanded.contains(&item.value);
                let active = context.selected == Some(&item.value);
                let key = item.key.unwrap();
                let has_children = !node.children.is_empty();
                let value = item.value.clone();
                let callback = context.on_select.clone();
                let enabled = context.enabled && item.enabled;
                let mut row: Element<C> = button("")
                    .accessible_label(bounded_semantic_name(&item.label, "Tree item"))
                    .key(&key)
                    .variant(if active {
                        ButtonVariant::Soft
                    } else {
                        ButtonVariant::Ghost
                    })
                    .width(
                        context
                            .width
                            .max(depth as f32 * context.indent_width + 104.),
                    )
                    .height(context.row_height)
                    .rounded(4.)
                    .enabled(enabled)
                    .opacity(if enabled { 1. } else { 0.45 })
                    .on_click(move |app: &mut C| {
                        if !active {
                            (callback.borrow_mut())(app, value.clone());
                        }
                    })
                    .into();
                let mut contents = Element::new(Kind::Row);
                contents.node.width = row.node.width;
                contents.node.height = Some(context.row_height);
                contents.node.align = Align::Center;
                row.node.access_role = Some(AccessRole::TreeItem);
                row.node.access_selected = Some(active);
                row.node.access_expanded = has_children.then_some(open);
                row.node.access_level = Some(depth);
                row.node.access_position = Some(position);
                row.node.access_set_size = has_children.then_some(node.children.len());
                row.node.access_parent = parent.map(str::to_owned);
                contents.children.push(
                    spacer()
                        .width(depth as f32 * context.indent_width)
                        .height(context.row_height)
                        .flex(0.)
                        .into(),
                );
                if has_children {
                    let value = item.value.clone();
                    let callback = context.on_expand.clone();
                    let mut disclosure: Element<C> = button("")
                        .accessible_label(bounded_semantic_name(
                            &format!(
                                "{} {}",
                                if open { "Collapse" } else { "Expand" },
                                item.label
                            ),
                            "Expand tree item",
                        ))
                        .key(format!("{key}-expand"))
                        .variant(ButtonVariant::Ghost)
                        .width(24.)
                        .height(28.)
                        .rounded(4.)
                        .enabled(enabled)
                        .on_click(move |app: &mut C| {
                            (callback.borrow_mut())(app, value.clone(), !open)
                        })
                        .into();
                    disclosure.node.align = Align::Center;
                    disclosure.node.flow = Some(Flow::Center);
                    // Stroke geometry is independent of font fallback/text insets.
                    disclosure.children.push(data_chevron(open, 12.));
                    contents.children.push(disclosure);
                } else {
                    contents
                        .children
                        .push(spacer().width(24.).height(28.).flex(0.).into());
                }
                let mut label = text(item.label);
                if active {
                    label = label.color(ColorToken::Primary).font_weight(500);
                }
                let mut content = Element::new(Kind::Row);
                content.node.height = Some(context.row_height);
                content.node.flex = 1.;
                content.node.align = Align::Center;
                content.node.padding = 6.;
                content.node.padding_top = Some(0.);
                content.node.padding_bottom = Some(0.);
                content.children.push(label.into());
                contents.children.push(content);
                row.children.push(contents);
                out.push(row);
                if open && !append(node.children, depth + 1, Some(&key), context, out) {
                    return false;
                }
            }
            true
        }
        let context = TreeContext {
            row_height: tree.1.row_height,
            indent_width: tree.1.indent_width,
            expanded: &tree.1.expanded,
            selected: tree.1.selected.as_ref(),
            enabled: tree.1.enabled,
            width,
            on_select: &on_select,
            on_expand: &on_expand,
        };
        if !append(tree.1.roots, 0, None, &context, &mut tree.0.children) {
            tree.0.validation_error = Some("tree visible row budget exceeds 96".into());
        }
        tree.0
    }
}

fn cell_flow(align: Align) -> Flow {
    match align {
        Align::End => Flow::Right,
        Align::Center => Flow::Center,
        _ => Flow::Left,
    }
}
fn data_rule<C>(width: f32) -> Element<C> {
    canvas(|c| {
        c.stroke(
            VectorPath::new().move_to(0., 0.5).line_to(width, 0.5),
            ColorToken::Border,
            1.,
        )
    })
    .width(width)
    .height(1.)
}
fn data_chevron<C>(down: bool, size: f32) -> Element<C> {
    canvas(|c| {
        let p = if down {
            VectorPath::new()
                .move_to(2.5, 4.5)
                .line_to(6., 8.)
                .line_to(9.5, 4.5)
        } else {
            VectorPath::new()
                .move_to(4.5, 2.5)
                .line_to(8., 6.)
                .line_to(4.5, 9.5)
        };
        c.stroke(
            p.transformed([size / 12., 0., 0., size / 12., 0., 0.]),
            ColorToken::Muted,
            1.5,
        );
    })
    .width(size)
    .height(size)
}

fn data_sort_chevron<C>(descending: bool) -> Element<C> {
    canvas(|c| {
        let y = if descending { [4., 8.] } else { [8., 4.] };
        c.stroke(
            VectorPath::new()
                .move_to(2.5, y[0])
                .line_to(6., y[1])
                .line_to(9.5, y[0]),
            ColorToken::Muted,
            1.5,
        );
    })
    .width(12.)
    .height(12.)
}
