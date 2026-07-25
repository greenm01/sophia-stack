use sophia_protocol::SurfaceId;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionStartupReadiness {
    pub surface: Option<SurfaceId>,
    pub client_focus_applied: bool,
    pub visual_detail: bool,
    pub stable_presented: bool,
    pub outputs_presented: bool,
    pub ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStartupEvent {
    PinSurface(SurfaceId),
    ClientFocusApplied(SurfaceId),
    VisualDetail(SurfaceId),
    StablePresented(SurfaceId),
    OutputsPresented,
    NativeRecovered,
    BlankSessionReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStartupUpdate {
    Ignored,
    Progressed,
    Ready,
    AlreadyReady,
}

pub fn reduce_session_startup(
    state: &mut SessionStartupReadiness,
    event: SessionStartupEvent,
) -> SessionStartupUpdate {
    if state.ready {
        return SessionStartupUpdate::AlreadyReady;
    }
    let progressed = match event {
        SessionStartupEvent::PinSurface(surface) => {
            if state.surface.is_some() {
                false
            } else {
                state.surface = Some(surface);
                true
            }
        }
        SessionStartupEvent::ClientFocusApplied(surface) => {
            update_surface_flag(state.surface, surface, &mut state.client_focus_applied)
        }
        SessionStartupEvent::VisualDetail(surface) => {
            update_surface_flag(state.surface, surface, &mut state.visual_detail)
        }
        SessionStartupEvent::StablePresented(surface) => {
            update_surface_flag(state.surface, surface, &mut state.stable_presented)
        }
        SessionStartupEvent::OutputsPresented => {
            let changed = !state.outputs_presented;
            state.outputs_presented = true;
            changed
        }
        SessionStartupEvent::NativeRecovered => {
            let changed = state.stable_presented || state.outputs_presented;
            state.stable_presented = false;
            state.outputs_presented = false;
            changed
        }
        SessionStartupEvent::BlankSessionReady => {
            state.ready = true;
            return SessionStartupUpdate::Ready;
        }
    };
    if state.surface.is_some()
        && state.client_focus_applied
        && state.visual_detail
        && state.stable_presented
        && state.outputs_presented
    {
        state.ready = true;
        SessionStartupUpdate::Ready
    } else if progressed {
        SessionStartupUpdate::Progressed
    } else {
        SessionStartupUpdate::Ignored
    }
}

fn update_surface_flag(pinned: Option<SurfaceId>, observed: SurfaceId, flag: &mut bool) -> bool {
    if pinned != Some(observed) || *flag {
        return false;
    }
    *flag = true;
    true
}
