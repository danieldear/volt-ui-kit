//! Controlled split panes with a genuine draggable, keyboard-focusable divider.
use super::*;
use crate::declarative::document::SplitProps;
pub struct SplitPane<C>(pub(super) Element<C>, SplitOptions);
struct SplitOptions {
    axis: Direction,
    ratio: f32,
    min_first: f32,
    min_second: f32,
    handle_size: f32,
    enabled: bool,
}
pub fn split<C>(first: impl Into<Element<C>>, second: impl Into<Element<C>>) -> SplitPane<C> {
    let mut e = Element::new(Kind::Row);
    e.node.width = Some(600.);
    e.node.height = Some(320.);
    e.children = vec![first.into(), second.into()];
    SplitPane(
        e,
        SplitOptions {
            axis: Direction::Horizontal,
            ratio: 0.5,
            min_first: 40.,
            min_second: 40.,
            handle_size: 8.,
            enabled: true,
        },
    )
}
impl<C> SplitPane<C> {
    pub fn axis(mut self, value: Direction) -> Self {
        self.1.axis = value;
        self
    }
    pub fn ratio(mut self, value: f32) -> Self {
        self.1.ratio = value;
        self
    }
    pub fn min_first(mut self, value: f32) -> Self {
        self.1.min_first = value;
        self
    }
    pub fn min_second(mut self, value: f32) -> Self {
        self.1.min_second = value;
        self
    }
    pub fn handle_size(mut self, value: f32) -> Self {
        self.1.handle_size = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.1.enabled = value;
        self
    }
    pub fn on_resize(mut self, f: impl FnMut(&mut C, f32) + 'static) -> Self {
        self.0.callback = Some(Callback::Number(Box::new(f)));
        self
    }
}
impl<C> From<SplitPane<C>> for Element<C> {
    fn from(mut split: SplitPane<C>) -> Self {
        if split.0.auto_id {
            split.0.validation_error = Some("split pane requires a stable .key(...)".into());
            return split.0;
        }
        split.0.node.kind = if split.1.axis == Direction::Horizontal {
            Kind::Row
        } else {
            Kind::Column
        };
        let mut handle = Element::new(Kind::Panel);
        handle.node.id = format!("{}-divider", split.0.node.id);
        handle.auto_id = false;
        handle.node.enabled = split.1.enabled;
        handle.node.split_handle = Some(split.0.node.id.clone());
        handle.node.style.radius = Some(Rounding::Radius(2.));
        handle.node.style.background = Some(ColorToken::Base300.into());
        split.0.children.insert(1, handle);
        split.0.node.split = Some(SplitProps {
            axis: split.1.axis,
            ratio: split.1.ratio,
            min_first: split.1.min_first,
            min_second: split.1.min_second,
            handle_size: split.1.handle_size,
            on_change: None,
        });
        split.0
    }
}
