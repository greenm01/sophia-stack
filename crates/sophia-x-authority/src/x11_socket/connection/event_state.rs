#[cfg(unix)]
fn x11_core_event_selection_update(
    request: &crate::XWireRequest,
) -> Option<(XResourceId, Option<u32>, Option<u32>)> {
    match request {
        crate::XWireRequest::CreateWindow {
            packet:
                crate::XAuthorityRequestPacket {
                    kind: crate::XAuthorityRequestKind::CreateWindow { window, .. },
                    ..
                },
            event_mask,
            do_not_propagate_mask,
            ..
        }
        | crate::XWireRequest::ChangeWindowAttributes {
            window,
            event_mask,
            do_not_propagate_mask,
        } => Some((*window, *event_mask, *do_not_propagate_mask)),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, Default)]
struct XCoreWindowEventSelection {
    mask: u32,
    do_not_propagate_mask: u32,
}

#[cfg(unix)]
#[derive(Debug)]
struct XCoreEventSelectionState {
    windows: BTreeMap<XResourceId, XCoreWindowEventSelection>,
    parents: BTreeMap<XResourceId, XResourceId>,
    stacking: Vec<XResourceId>,
    mapped: BTreeSet<XResourceId>,
    fallback_mapped_window: XResourceId,
}

#[cfg(unix)]
impl Default for XCoreEventSelectionState {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            parents: BTreeMap::new(),
            stacking: Vec::new(),
            mapped: BTreeSet::new(),
            fallback_mapped_window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
        }
    }
}

#[cfg(unix)]
impl XCoreEventSelectionState {
    const KEY_MASKS: u32 = (1 << 0) | (1 << 1);

    fn update(
        &mut self,
        window: XResourceId,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    ) {
        if event_mask.is_none() && do_not_propagate_mask.is_none() {
            return;
        }
        let selection = self.windows.entry(window).or_default();
        if let Some(mask) = event_mask {
            selection.mask = mask;
        }
        if let Some(mask) = do_not_propagate_mask {
            selection.do_not_propagate_mask = mask;
        }
    }

    fn register(&mut self, window: XResourceId, parent: XResourceId) {
        self.parents.insert(window, parent);
        self.stacking.retain(|candidate| *candidate != window);
        self.stacking.push(window);
    }

    fn restack(&mut self, window: XResourceId, sibling: Option<XResourceId>, mode: Option<u8>) {
        self.stacking.retain(|candidate| *candidate != window);
        let sibling_index = sibling.and_then(|sibling| {
            self.stacking
                .iter()
                .position(|candidate| *candidate == sibling)
        });
        let index = match (mode, sibling_index) {
            (Some(1 | 3), Some(index)) => index,
            (Some(1 | 3), None) => 0,
            (Some(0 | 2 | 4), Some(index)) => index.saturating_add(1),
            _ => self.stacking.len(),
        };
        self.stacking.insert(index.min(self.stacking.len()), window);
    }

    fn observe_mapped(&mut self, window: XResourceId) {
        self.mapped.insert(window);
        self.fallback_mapped_window = window;
    }

    fn observe_unmapped(&mut self, window: XResourceId) {
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = self
                .stacking
                .iter()
                .rev()
                .copied()
                .find(|candidate| self.mapped.contains(candidate))
                .unwrap_or_else(|| XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1));
        }
    }

    fn remove(&mut self, window: XResourceId) {
        self.windows.remove(&window);
        self.parents.remove(&window);
        self.stacking.retain(|candidate| *candidate != window);
        self.mapped.remove(&window);
        if self.fallback_mapped_window == window {
            self.fallback_mapped_window = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        }
    }

    fn keyboard_target(&self, focused: XResourceId) -> XResourceId {
        self.selected_keyboard_target(focused)
            .unwrap_or_else(|| self.keyboard_fallback(focused))
    }

    fn selected_keyboard_target(&self, focused: XResourceId) -> Option<XResourceId> {
        let mut candidate = self.keyboard_fallback(focused);
        for _ in 0..64 {
            if self
                .windows
                .get(&candidate)
                .is_some_and(|selection| selection.mask & Self::KEY_MASKS != 0)
            {
                return Some(candidate);
            }
            candidate = self.parents.get(&candidate).copied()?;
        }
        None
    }

    fn keyboard_fallback(&self, focused: XResourceId) -> XResourceId {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        if focused == root {
            self.stacking
                .iter()
                .rev()
                .copied()
                .find(|window| self.mapped.contains(window))
                .unwrap_or(self.fallback_mapped_window)
        } else {
            focused
        }
    }

    fn ancestors(&self, window: XResourceId) -> Vec<XResourceId> {
        let mut ancestors = Vec::new();
        let mut candidate = window;
        for _ in 0..64 {
            let Some(parent) = self.parents.get(&candidate).copied() else {
                break;
            };
            ancestors.push(parent);
            candidate = parent;
        }
        ancestors
    }
}
