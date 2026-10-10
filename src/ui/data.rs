//! Bounded data composition. Selection and expansion remain application-owned.
use super::*;
use crate::declarative::document::VirtualListProps;
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
            list.0.children.push(row.into());
        }
        list.0.node.virtual_list = Some(VirtualListProps {
            count: list.1.count,
            start: window.start,
            end: window.end,
            row_height: list.1.row_height,
            overscan: list.1.overscan,
            on_window_change: None,
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
}
pub fn table_column<K>(value: K, label: impl Into<String>) -> TableColumn<K> {
    TableColumn {
        value,
        label: label.into(),
        width: 120.,
        sortable: true,
    }
}
impl<K> TableColumn<K> {
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
    e.node.surface.scroll = Some(Scroll::Vertical);
    Table(
        e,
        TableOptions {
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
            || 2 + columns.len() + rows.len() * (3 + columns.len()) >= MAX_NODES
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
        let width = columns.iter().map(|c| c.width).sum::<f32>();
        table.0.node.width = table.0.node.width.or(Some(width));
        if let Some((key, direction)) = &table.1.sort {
            let index = columns
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
        let mut header = Element::new(Kind::Row);
        header.node.height = Some(36.);
        header.node.width = Some(width);
        header.node.style.background = Some(ColorToken::Base200.into());
        for (i, column) in table.1.columns.into_iter().enumerate() {
            let selected = table
                .1
                .sort
                .as_ref()
                .is_some_and(|(key, _)| key == &column.value);
            let direction = if selected {
                table.1.sort.as_ref().unwrap().1.reversed()
            } else {
                SortDirection::Ascending
            };
            let suffix = if selected {
                if direction == SortDirection::Descending {
                    " ↑"
                } else {
                    " ↓"
                }
            } else {
                ""
            };
            let callback = table.1.on_sort.clone();
            let key = column.value;
            // Only sortable columns are buttons; a fixed column is a label, not
            // a disabled control.
            let cell: Element<C> = if column.sortable {
                button(format!("{}{suffix}", column.label))
                    .key(format!("{}-column-{i}", table.0.node.id))
                    .variant(ButtonVariant::Ghost)
                    .label_align(Align::Start)
                    .color(ColorToken::Muted)
                    .font_size(12.)
                    .font_weight(600)
                    .width(column.width)
                    .height(36.)
                    .rounded(0.)
                    .enabled(table.1.enabled)
                    .on_click(move |app: &mut C| {
                        if let Some(callback) = &callback {
                            (callback.borrow_mut())(app, key.clone(), direction);
                        }
                    })
                    .into()
            } else {
                let mut label = Element::new(Kind::Row);
                label.node.width = Some(column.width);
                label.node.height = Some(36.);
                label.node.padding = 10.;
                label.node.align = Align::Center;
                label.children.push(
                    text(column.label)
                        .color(ColorToken::Muted)
                        .font_size(12.)
                        .font_weight(600)
                        .into(),
                );
                label
            };
            header.children.push(cell);
        }
        table.0.children.push(header);
        let on_select = table.1.on_select.unwrap();
        // Rows overlay a full-width hit target with non-interactive text cells.
        for row in table.1.rows {
            let active = table.1.selected.as_ref() == Some(&row.value);
            let handler = on_select.clone();
            let value = row.value;
            let key = row.key.unwrap();
            let mut e = Element::new(Kind::ZStack);
            e.node.width = Some(width);
            e.node.height = Some(34.);
            e.node.align = Align::Stretch;
            let hit: Element<C> = button("")
                .key(key)
                .variant(if active {
                    ButtonVariant::Soft
                } else {
                    ButtonVariant::Ghost
                })
                .rounded(0.)
                .enabled(table.1.enabled && row.enabled)
                .on_click(move |app: &mut C| {
                    if !active {
                        (handler.borrow_mut())(app, value.clone());
                    }
                })
                .into();
            e.children.push(hit);
            let mut cells = Element::new(Kind::Row);
            cells.node.height = Some(34.);
            cells.node.width = Some(width);
            cells.node.align = Align::Center;
            // 10px insets line cell text up with the header labels.
            cells.node.padding = 10.;
            cells.node.gap = 20.;
            // Widths were already validated; use the corresponding header width.
            for (i, label) in row.cells.into_iter().enumerate() {
                let width = table.0.children[0].children[i].node.width.unwrap();
                let mut cell = text(label).width((width - 20.).max(1.));
                if active {
                    cell = cell.color(ColorToken::Primary).font_weight(600);
                }
                cells.children.push(cell.into());
            }
            e.children.push(cells);
            table.0.children.push(e);
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
    TreeView(
        e,
        TreeOptions {
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
        let on_select = tree.1.on_select.unwrap();
        let on_expand = tree.1.on_expand.unwrap();
        let width = tree.0.node.width.unwrap_or(360.);
        struct TreeContext<'a, C, T> {
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
            context: &TreeContext<'_, C, T>,
            out: &mut Vec<Element<C>>,
        ) -> bool {
            for node in nodes {
                if out.len() >= 96 {
                    return false;
                }
                let item = node.item;
                let open = context.expanded.contains(&item.value);
                let active = context.selected == Some(&item.value);
                let key = item.key.unwrap();
                let mut row = Element::new(Kind::Row);
                row.node.width = Some(context.width);
                row.node.height = Some(32.);
                row.children.push(
                    spacer()
                        .width(depth as f32 * 14.)
                        .height(32.)
                        .flex(0.)
                        .into(),
                );
                if node.children.is_empty() {
                    row.children
                        .push(spacer().width(22.).height(32.).flex(0.).into());
                } else {
                    let value = item.value.clone();
                    let callback = context.on_expand.clone();
                    row.children.push(
                        button(if open { "▾" } else { "▸" })
                            .key(format!("{key}-expand"))
                            .variant(ButtonVariant::Ghost)
                            .color(ColorToken::Muted)
                            .width(22.)
                            .height(32.)
                            .enabled(context.enabled && item.enabled)
                            .on_click(move |app: &mut C| {
                                (callback.borrow_mut())(app, value.clone(), !open)
                            })
                            .into(),
                    );
                }
                let value = item.value;
                let callback = context.on_select.clone();
                let mut label = button(item.label)
                    .key(key)
                    .variant(if active {
                        ButtonVariant::Soft
                    } else {
                        ButtonVariant::Ghost
                    })
                    .label_align(Align::Start)
                    .height(32.)
                    .flex(1.)
                    .enabled(context.enabled && item.enabled)
                    .on_click(move |app: &mut C| {
                        if !active {
                            (callback.borrow_mut())(app, value.clone());
                        }
                    });
                if active {
                    label = label.font_weight(600);
                }
                row.children.push(label.into());
                out.push(row);
                if open && !append(node.children, depth + 1, context, out) {
                    return false;
                }
            }
            true
        }
        let context = TreeContext {
            expanded: &tree.1.expanded,
            selected: tree.1.selected.as_ref(),
            enabled: tree.1.enabled,
            width,
            on_select: &on_select,
            on_expand: &on_expand,
        };
        if !append(tree.1.roots, 0, &context, &mut tree.0.children) {
            tree.0.validation_error = Some("tree visible row budget exceeds 96".into());
        }
        tree.0
    }
}
