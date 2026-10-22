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
}
