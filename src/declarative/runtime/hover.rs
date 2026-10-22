//! `on_hover` regions: containers told when the pointer enters or leaves
//! their bounds, for surfaces that stay open while the pointer is over them.
use super::*;

impl Runtime {
    /// Called by hosts after pointer motion, with `None` once the pointer
    /// has left the window: reports each `on_hover` container the pointer
    /// entered or left since the last call. A region outside an open modal
    /// scope counts as left. Regions that left the document are forgotten
    /// without a report. Returns whether any handler ran (the host then
    /// recomposes).
    pub fn dispatch_hover_regions<C>(
        &mut self,
        pointer: Option<[f32; 2]>,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let mut names = BTreeMap::new();
        let mut inside = BTreeSet::new();
        for b in &self.boxes {
            let Some(name) = &b.on_hover else {
                continue;
            };
            names.insert(b.id.clone(), name.clone());
            let Some([x, y]) = pointer else {
                continue;
            };
            if self.scope_allows_id(&b.id)
                && b.clip.contains(x, y)
                && b.clip_regions.iter().all(|r| r.contains(x, y))
                && b.bounds.contains_corners(b.radii(self.scale), x, y)
            {
                inside.insert(b.id.clone());
            }
        }
        let mut events = Vec::new();
        for id in inside.difference(&self.hover_regions) {
            events.push((names[id].clone(), true));
        }
        for id in self.hover_regions.difference(&inside) {
            if let Some(name) = names.get(id) {
                events.push((name.clone(), false));
            }
        }
        self.hover_regions = inside;
        let any = !events.is_empty();
        for (name, entered) in events {
            handlers
                .dispatch(&name, Event::ToggleChanged(entered), context)
                .map_err(|e| DocumentError(e.to_string()))?;
        }
        Ok(any)
    }

    /// Called by hosts after input that may have moved keyboard focus:
    /// reports each `on_focus_within` container that focus entered (it or a
    /// descendant is focused) or left since the last call. Returns whether
    /// any handler ran.
    pub fn dispatch_focus_regions<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let mut names = BTreeMap::new();
        for b in &self.boxes {
            if let Some(name) = &b.on_focus_within {
                names.insert(b.id.clone(), name.clone());
            }
        }
        let mut inside = BTreeSet::new();
        if let Some(mut index) = self
            .focused
            .as_deref()
            .and_then(|id| self.boxes.iter().position(|b| b.id == id))
        {
            loop {
                let b = &self.boxes[index];
                if names.contains_key(&b.id) {
                    inside.insert(b.id.clone());
                }
                if b.depth == 0 {
                    break;
                }
                let depth = b.depth - 1;
                let Some(parent) = self.boxes[..index].iter().rposition(|p| p.depth == depth) else {
                    break;
                };
                index = parent;
            }
        }
        let mut events = Vec::new();
        for id in inside.difference(&self.focus_regions) {
            events.push((names[id].clone(), true));
        }
        for id in self.focus_regions.difference(&inside) {
            if let Some(name) = names.get(id) {
                events.push((name.clone(), false));
            }
        }
        self.focus_regions = inside;
        let any = !events.is_empty();
        for (name, entered) in events {
            handlers
                .dispatch(&name, Event::ToggleChanged(entered), context)
                .map_err(|e| DocumentError(e.to_string()))?;
        }
        Ok(any)
    }

    /// Called by hosts for an Escape nothing else used (no editor, menu or
    /// dialog took it): the last `on_escape` container in the document that
    /// the open scope allows gets it. Returns whether one did.
    pub fn dispatch_escape<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<bool, DocumentError> {
        let Some(name) = self
            .boxes
            .iter()
            .rev()
            .find(|b| b.on_escape.is_some() && self.scope_allows_id(&b.id))
            .and_then(|b| b.on_escape.clone())
        else {
            return Ok(false);
        };
        handlers
            .dispatch(&name, Event::Activate, context)
            .map_err(|e| DocumentError(e.to_string()))?;
        Ok(true)
    }
}
