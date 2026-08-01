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
            ..
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
#[derive(Clone, Copy, Debug)]
struct XCorePointerSnapshot {
    surface_window: XResourceId,
    delivered_window: XResourceId,
    root_x: i16,
    root_y: i16,
    event_x: i16,
    event_y: i16,
    mask: u16,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct XCorePointerQuery {
    child: XResourceId,
    root_x: i16,
    root_y: i16,
    win_x: i16,
    win_y: i16,
    mask: u16,
}

#[cfg(unix)]
#[derive(Debug)]
struct XCoreEventSelectionState {
    windows: BTreeMap<XResourceId, XCoreWindowEventSelection>,
    parents: BTreeMap<XResourceId, XResourceId>,
    geometries: BTreeMap<XResourceId, Rect>,
    stacking: Vec<XResourceId>,
    mapped: BTreeSet<XResourceId>,
    fallback_mapped_window: XResourceId,
    pointer: Option<XCorePointerSnapshot>,
}

#[cfg(unix)]
impl Default for XCoreEventSelectionState {
    fn default() -> Self {
        Self {
            windows: BTreeMap::new(),
            parents: BTreeMap::new(),
            geometries: BTreeMap::new(),
            stacking: Vec::new(),
            mapped: BTreeSet::new(),
            fallback_mapped_window: XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1),
            pointer: None,
        }
    }
}

#[cfg(unix)]
impl XCoreEventSelectionState {
    const KEY_MASKS: u32 = (1 << 0) | (1 << 1);
    const BUTTON_MASKS: u32 = (1 << 2) | (1 << 3);
    const POINTER_MOTION_MASK: u32 = 1 << 6;
    const ENTER_WINDOW_MASK: u32 = 1 << 4;
    const LEAVE_WINDOW_MASK: u32 = 1 << 5;

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

    fn register(&mut self, window: XResourceId, parent: XResourceId, geometry: Rect) {
        self.parents.insert(window, parent);
        self.geometries.insert(window, geometry);
        self.stacking.retain(|candidate| *candidate != window);
        self.stacking.push(window);
    }

    fn reparent(&mut self, window: XResourceId, parent: XResourceId, x: i16, y: i16) {
        self.parents.insert(window, parent);
        if let Some(geometry) = self.geometries.get_mut(&window) {
            geometry.x = i32::from(x);
            geometry.y = i32::from(y);
        }
    }

    fn configure_geometry(
        &mut self,
        window: XResourceId,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
    ) {
        let geometry = self.geometries.entry(window).or_insert(Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        });
        if let Some(x) = x {
            geometry.x = i32::from(x);
        }
        if let Some(y) = y {
            geometry.y = i32::from(y);
        }
        if let Some(width) = width {
            geometry.width = i32::from(width);
        }
        if let Some(height) = height {
            geometry.height = i32::from(height);
        }
    }

    fn update_geometry(&mut self, window: XResourceId, geometry: Rect) {
        self.geometries.insert(window, geometry);
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
        self.geometries.remove(&window);
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

    fn selected_pointer_target(
        &self,
        surface_window: XResourceId,
        motion: bool,
        event_x: i16,
        event_y: i16,
    ) -> Option<XResourceId> {
        let selected_mask = if motion {
            Self::POINTER_MOTION_MASK
        } else {
            Self::BUTTON_MASKS
        };
        self.stacking.iter().rev().copied().find(|candidate| {
            self.mapped.contains(candidate)
                && (*candidate == surface_window
                    || self.ancestors(*candidate).contains(&surface_window))
                && self.contains_surface_point(
                    surface_window,
                    *candidate,
                    i32::from(event_x),
                    i32::from(event_y),
                )
                && self
                    .windows
                    .get(candidate)
                    .is_some_and(|selection| selection.mask & selected_mask != 0)
        })
    }

    fn crossing_selected(&self, window: XResourceId, entered: bool) -> bool {
        let mask = if entered {
            Self::ENTER_WINDOW_MASK
        } else {
            Self::LEAVE_WINDOW_MASK
        };
        self.windows
            .get(&window)
            .is_some_and(|selection| selection.mask & mask != 0)
    }

    fn pointer_event_coordinates(
        &self,
        surface_window: XResourceId,
        delivered_window: XResourceId,
        event_x: i16,
        event_y: i16,
    ) -> (i16, i16) {
        let Some((surface_x, surface_y)) = self.root_origin(surface_window) else {
            return (event_x, event_y);
        };
        let Some((delivered_x, delivered_y)) = self.root_origin(delivered_window) else {
            return (event_x, event_y);
        };
        (
            clamp_engine_i16(i32::from(event_x) + surface_x - delivered_x),
            clamp_engine_i16(i32::from(event_y) + surface_y - delivered_y),
        )
    }

    fn contains_surface_point(
        &self,
        surface_window: XResourceId,
        candidate: XResourceId,
        event_x: i32,
        event_y: i32,
    ) -> bool {
        if candidate == surface_window {
            return true;
        }
        let Some(geometry) = self.geometries.get(&candidate) else {
            return true;
        };
        let Some((surface_x, surface_y)) = self.root_origin(surface_window) else {
            return true;
        };
        let Some((candidate_x, candidate_y)) = self.root_origin(candidate) else {
            return true;
        };
        let local_x = event_x + surface_x - candidate_x;
        let local_y = event_y + surface_y - candidate_y;
        local_x >= 0
            && local_y >= 0
            && local_x < geometry.width
            && local_y < geometry.height
    }

    fn root_origin(&self, window: XResourceId) -> Option<(i32, i32)> {
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        let mut candidate = window;
        let mut x = 0_i32;
        let mut y = 0_i32;
        for _ in 0..64 {
            if candidate == root {
                return Some((x, y));
            }
            let geometry = self.geometries.get(&candidate)?;
            x = x.saturating_add(geometry.x);
            y = y.saturating_add(geometry.y);
            candidate = self.parents.get(&candidate).copied()?;
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_pointer(
        &mut self,
        surface_window: XResourceId,
        delivered_window: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        mask: u16,
    ) {
        self.pointer = Some(XCorePointerSnapshot {
            surface_window,
            delivered_window,
            root_x,
            root_y,
            event_x,
            event_y,
            mask,
        });
    }

    fn query_pointer(&self, window: XResourceId) -> Option<XCorePointerQuery> {
        let pointer = self.pointer?;
        let root = XResourceId::new(u64::from(X_SETUP_DEFAULT_ROOT), 1);
        let (child, win_x, win_y) = if window == root {
            (pointer.surface_window, pointer.root_x, pointer.root_y)
        } else if window == pointer.surface_window {
            (
                (pointer.delivered_window != pointer.surface_window)
                    .then_some(pointer.delivered_window)
                    .unwrap_or(XResourceId::NONE),
                pointer.event_x,
                pointer.event_y,
            )
        } else {
            (XResourceId::NONE, pointer.event_x, pointer.event_y)
        };
        Some(XCorePointerQuery {
            child,
            root_x: pointer.root_x,
            root_y: pointer.root_y,
            win_x,
            win_y,
            mask: pointer.mask,
        })
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
