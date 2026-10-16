//! A two-column app shell, like SwiftUI's `NavigationSplitView`: a sidebar
//! beside a detail column with a toolbar. It draws like the Catalog by
//! default and stays optional; apps can still compose their own shell.
use super::*;
use std::{cell::RefCell, rc::Rc};

/// Where the sidebar is being built: in its column, or in the drawer that
/// replaces the column in narrow windows. Both exist at once, so keys inside
/// the sidebar must differ between them; [`SidebarPlace::key`] does that.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarPlace {
    Column,
    Drawer,
}
impl SidebarPlace {
    /// `base` in the column, `base-drawer` in the drawer.
    pub fn key(self, base: &str) -> String {
        match self {
            Self::Column => base.to_owned(),
            Self::Drawer => format!("{base}-drawer"),
        }
    }
}
type DrawerChange<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
type Resize<C> = Box<dyn FnMut(&mut C, f32)>;
pub struct NavigationSplitView<C>(pub(super) Element<C>, SplitViewOptions<C>);
struct SplitViewOptions<C> {
    column: Element<C>,
    drawer: Element<C>,
    detail: Element<C>,
    header: Option<Element<C>>,
    footer: Option<Element<C>>,
    toolbar: Option<Element<C>>,
    title: Option<String>,
    drawer_open: bool,
    on_drawer: Option<DrawerChange<C>>,
    width: f32,
    on_resize: Option<Resize<C>>,
    collapse_below: Breakpoint,
    titlebar_inset: bool,
    material: bool,
}
/// A sidebar beside a detail column. `sidebar` is called once for the
/// sidebar column and once for the narrow-window drawer (see
/// [`SidebarPlace`]); `detail` fills the column under the toolbar.
///
/// Defaults follow the Catalog: a 248pt sidebar on `Base200`, a `Base100`
/// detail sheet, a 56pt draggable toolbar, room for the macOS window controls
/// over the sidebar, and, below `Lg`, the sidebar collapses behind a menu
/// button that opens it as a drawer (wire `.drawer_open` and
/// `.on_drawer_change`).
pub fn navigation_split_view<C, E>(
    mut sidebar: impl FnMut(SidebarPlace) -> E,
    detail: impl Into<Element<C>>,
) -> NavigationSplitView<C>
where
    E: Into<Element<C>>,
{
    NavigationSplitView(
        Element::new(Kind::ZStack),
        SplitViewOptions {
            column: sidebar(SidebarPlace::Column).into(),
            drawer: sidebar(SidebarPlace::Drawer).into(),
            detail: detail.into(),
            header: None,
            footer: None,
            toolbar: None,
            title: None,
            drawer_open: false,
            on_drawer: None,
            width: 248.,
            on_resize: None,
            collapse_below: Breakpoint::Lg,
            titlebar_inset: cfg!(target_os = "macos"),
            material: false,
        },
    )
}
impl<C> NavigationSplitView<C> {
    /// Above the sidebar's scrolling content, e.g. the app's name or logo.
    pub fn sidebar_header(mut self, header: impl Into<Element<C>>) -> Self {
        self.1.header = Some(header.into());
        self
    }
    /// Below the sidebar's scrolling content.
    pub fn sidebar_footer(mut self, footer: impl Into<Element<C>>) -> Self {
        self.1.footer = Some(footer.into());
        self
    }
    /// The toolbar row above the detail; it fills the row's width.
    pub fn toolbar(mut self, toolbar: impl Into<Element<C>>) -> Self {
        self.1.toolbar = Some(toolbar.into());
        self
    }
    /// The drawer's title in narrow windows.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.1.title = Some(title.into());
        self
    }
    /// Whether the narrow-window drawer is open (app-owned state).
    pub fn drawer_open(mut self, open: bool) -> Self {
        self.1.drawer_open = open;
        self
    }
    /// The menu button opens the drawer (`true`); Escape, the backdrop or
    /// its close button dismiss it (`false`). Without this callback narrow
    /// windows simply hide the sidebar.
    pub fn on_drawer_change(mut self, f: impl FnMut(&mut C, bool) + 'static) -> Self {
        self.1.on_drawer = Some(Rc::new(RefCell::new(f)));
        self
    }
    /// The sidebar's width in logical pixels (default 248).
    pub fn sidebar_width(mut self, width: f32) -> Self {
        self.1.width = width;
        self
    }
    /// Make the divider draggable (200–420pt); store the width and pass it
    /// back through `.sidebar_width`.
    pub fn on_sidebar_resize(mut self, f: impl FnMut(&mut C, f32) + 'static) -> Self {
        self.1.on_resize = Some(Box::new(f));
        self
    }
    /// The window width below which the sidebar moves into the drawer
    /// (default `Lg`).
    pub fn collapse_below(mut self, breakpoint: Breakpoint) -> Self {
        self.1.collapse_below = breakpoint;
        self
    }
    /// Reserve the top of the sidebar for the macOS window controls (the
    /// default on macOS, for `WindowChrome::Overlay`). Turn it off with
    /// native window chrome or when the view is not the window's root.
    pub fn titlebar_inset(mut self, inset: bool) -> Self {
        self.1.titlebar_inset = inset;
        self
    }
    /// Let a translucent window background show through the sidebar; the
    /// detail floats on it as an inset, shadowed sheet.
    pub fn material(mut self, material: bool) -> Self {
        self.1.material = material;
        self
    }
}
impl<C: 'static> From<NavigationSplitView<C>> for Element<C> {
    fn from(mut view: NavigationSplitView<C>) -> Self {
        if view.0.auto_id {
            view.0.validation_error =
                Some("navigation split view requires a stable .key(...)".into());
            return view.0;
        }
        let id = view.0.node.id.clone();
        let o = view.1;
        let wide = responsive(false).at(o.collapse_below, true);
        let narrow = responsive(true).at(o.collapse_below, false);
        let clear = || ColorToken::BaseContent.alpha(0);

        // The sidebar column: window-control lane, header, scrolling
        // content, footer.
        let mut column = vec![vstack([])
            .key(format!("{id}-titlebar"))
            .height(if o.titlebar_inset { 48. } else { 8. })
            .drag_region(true)
            .into()];
        column.extend(o.header);
        column.push(
            vstack([o.column])
                .key(format!("{id}-sidebar"))
                .flex(1.)
                .align(Align::Stretch)
                .padding(12., Edge::Horizontal)
                .padding(8., Edge::Top)
                .padding(16., Edge::Bottom)
                .scroll(Scroll::Vertical)
                .scrollbars(Scrollbars::Hidden)
                .into(),
        );
        column.extend(o.footer);
        let column = vstack(column)
            .key(format!("{id}-sidebar-column"))
            .align(Align::Stretch)
            .visible(wide);

        // The toolbar: in narrow windows a menu button, clear of the macOS
        // window controls, opens the drawer.
        let mut bar: Vec<Element<C>> = vec![];
        if let Some(change) = o.on_drawer.clone() {
            let lane = if o.titlebar_inset { 64. } else { 0. };
            bar.push(
                hstack([
                    spacer().width(lane).flex(0.).into(),
                    zstack([
                        button("")
                            .key(format!("{id}-menu"))
                            .accessible_label("Show sidebar")
                            .variant(ButtonVariant::Ghost)
                            .width(36.)
                            .height(36.)
                            .on_click(move |app: &mut C| (change.borrow_mut())(app, true))
                            .into(),
                        icon(Icon::Menu).size(18.).into(),
                    ])
                    .width(36.)
                    .height(36.)
                    .flow(Flow::Center)
                    .into(),
                ])
                .key(format!("{id}-menu-lane"))
                .align(Align::Center)
                // Rigid: a wide toolbar must not squeeze it.
                .min_width(lane + 36.)
                .visible(narrow)
                .into(),
            );
        }
        if let Some(mut toolbar) = o.toolbar {
            toolbar.node.flex = 1.;
            bar.push(toolbar);
        }
        let toolbar = hstack(bar)
            .key(format!("{id}-toolbar"))
            .height(56.)
            .gap(6., Axis::Horizontal)
            .padding(16., Edge::Horizontal)
            .align(Align::Center)
            .drag_region(true);

        let mut detail_content = o.detail;
        if detail_content.node.flex == 0. {
            detail_content.node.flex = 1.;
        }
        let detail = vstack([
            toolbar.into(),
            divider().background(ColorToken::Border).into(),
            detail_content,
        ])
        .key(format!("{id}-detail"))
        .flex(1.)
        .align(Align::Stretch)
        .background(ColorToken::Base100)
        .border_color(if o.material {
            ColorToken::BaseContent.alpha(9)
        } else {
            ColorToken::Border.into()
        })
        .border_width(1.);
        // On a material the detail is a sheet inset from every free edge,
        // so it never meets the window's rounded corners.
        let detail: Element<C> = if o.material {
            vstack([
                vstack([])
                    .key(format!("{id}-sheet-drag"))
                    .height(8.)
                    .drag_region(true)
                    .into(),
                detail
                    .rounded(12.)
                    .shadow(Shadow::new().color("#0000002E").blur(18.).offset(0., 2.))
                    .into(),
            ])
            .key(format!("{id}-sheet"))
            .flex(1.)
            .align(Align::Stretch)
            .padding(8., Edge::Right)
            .padding(8., Edge::Bottom)
            .into()
        } else {
            detail.into()
        };

        let surface = if o.material {
            clear()
        } else {
            ColorToken::Base200.into()
        };
        let body: Element<C> = match o.on_resize {
            // A wide invisible grab area; a pill appears on hover or drag.
            Some(mut resize) => split(column, detail)
                .key(format!("{id}-split"))
                .first_size(o.width)
                .min_first(200.)
                .max_first(420.)
                .min_second(480.)
                .handle_size(10.)
                .handle_color(clear())
                .handle_style(SplitHandle::Reveal)
                .fill_width()
                .fill_height()
                .flex(1.)
                .background(surface.clone())
                .on_resize(move |app: &mut C, width| resize(app, width))
                .into(),
            // Rigid: a wide detail must not squeeze the sidebar.
            None => hstack([column.width(o.width).min_width(o.width).into(), detail])
                .key(format!("{id}-split"))
                .flex(1.)
                .align(Align::Stretch)
                .background(surface.clone())
                .into(),
        };
        let mut layers = vec![body];
        if let Some(change) = o.on_drawer {
            let mut panel = drawer(o.drawer)
                .key(format!("{id}-drawer"))
                .side(DrawerSide::Left)
                .size(280.)
                .open(o.drawer_open)
                .on_dismiss(move |app: &mut C| (change.borrow_mut())(app, false));
            if let Some(title) = o.title {
                panel = panel.title(title);
            }
            layers.push(panel.into());
        }
        view.0.children = layers;
        view.0.node.align = Align::Stretch;
        if view.0.node.style.background.is_none() {
            view.0.node.style.background = Some(surface);
        }
        view.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(e: &Element<()>, out: &mut Vec<String>) {
        out.push(e.node.id.clone());
        for child in &e.children {
            ids(child, out);
        }
    }
    fn all(e: &Element<()>) -> Vec<String> {
        let mut out = vec![];
        ids(e, &mut out);
        out
    }
    fn shell() -> NavigationSplitView<()> {
        navigation_split_view(
            |place| {
                navigation([navigation_item(0u8, "Inbox").key("inbox")])
                    .key(place.key("nav"))
                    .on_select(|_: &mut (), _| {})
            },
            text("Detail").key("page"),
        )
        .key("app")
    }
    #[test]
    fn requires_a_stable_key() {
        let e: Element<()> = navigation_split_view(|_| text("Side"), text("Detail")).into();
        assert!(e.build().is_err());
    }
    #[test]
    fn builds_a_sidebar_column_beside_a_detail_with_a_toolbar() {
        let e: Element<()> = shell().toolbar(text("Inbox").key("title")).into();
        let ids = all(&e);
        for id in [
            "app-split",
            "app-sidebar-column",
            "app-titlebar",
            "app-sidebar",
            "nav-inbox",
            "app-detail",
            "app-toolbar",
            "title",
            "page",
        ] {
            assert!(ids.iter().any(|i| i == id), "{id} in {ids:?}");
        }
        // No drawer wiring: no menu button and no drawer.
        assert!(!ids.iter().any(|i| i == "app-menu" || i == "app-drawer"));
        assert_eq!(e.node.style.background.as_deref(), Some("@base-200"));
        assert!(e.build().is_ok());
    }
    #[test]
    fn the_drawer_gets_its_own_copy_of_the_sidebar() {
        let e: Element<()> = shell()
            .title("Mail")
            .drawer_open(true)
            .on_drawer_change(|_: &mut (), _| {})
            .into();
        let ids = all(&e);
        assert!(ids.iter().any(|i| i == "app-menu"));
        assert!(ids.iter().any(|i| i == "nav-inbox"));
        assert!(ids.iter().any(|i| i == "nav-drawer-inbox"));
        assert!(e.build().is_ok(), "keys stay unique");
    }
    #[test]
    fn resizing_uses_a_split_with_a_divider() {
        let fixed: Element<()> = shell().sidebar_width(300.).into();
        assert_eq!(fixed.children[0].children[0].node.width, Some(300.));
        let resizable: Element<()> = shell()
            .sidebar_width(300.)
            .on_sidebar_resize(|_: &mut (), _| {})
            .into();
        assert!(all(&resizable).iter().any(|i| i == "app-split-divider"));
        assert!(resizable.build().is_ok());
    }
    #[test]
    fn material_floats_the_detail_on_a_clear_window() {
        let e: Element<()> = shell().material(true).into();
        assert_eq!(e.node.style.background.as_deref(), Some("@base-content/0"));
        assert!(all(&e).iter().any(|i| i == "app-sheet"));
    }
    #[test]
    fn the_titlebar_lane_clears_the_window_controls_only_when_asked() {
        let lane = |inset: bool| {
            let e: Element<()> = shell().titlebar_inset(inset).into();
            e.children[0].children[0].children[0].node.height
        };
        assert_eq!(lane(true), Some(48.));
        assert_eq!(lane(false), Some(8.));
    }
}
