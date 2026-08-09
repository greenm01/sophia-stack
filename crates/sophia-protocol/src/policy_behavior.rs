/// Stable names for the revision-1 black-box behavior corpus. Every public
/// policy client must accept the same complete snapshots and return a proposal
/// admitted by the canonical reducer for each entry before revision 1 freezes.
pub const SOPHIA_WM_V1_BEHAVIOR_SCENARIOS: [&str; 4] = [
    "single-output-constraints",
    "two-output-partition",
    "output-loss",
    "returned-output-generation",
];

/// Returns the canonical complete scene for one revision-1 behavior scenario.
/// The sequence is intentionally stateful when consumed in declaration order.
pub fn sophia_wm_v1_behavior_scene(scenario: &str) -> Option<crate::PolicySceneSnapshot> {
    let first = crate::OutputId::from_raw(1);
    let second = crate::OutputId::from_raw(2);
    let (generation, active_output, outputs, surfaces) = match scenario {
        "single-output-constraints" => (
            1,
            first,
            vec![output(first, 1, 0, Some(crate::SurfaceId::new(3, 1)))],
            vec![surface(3, 1, first), surface(4, 1, first)],
        ),
        "two-output-partition" => (
            2,
            second,
            vec![
                output(first, 1, 0, Some(crate::SurfaceId::new(3, 1))),
                output(second, 1, 1200, Some(crate::SurfaceId::new(5, 1))),
            ],
            vec![
                surface(3, 1, first),
                surface(4, 1, first),
                surface(5, 1, second),
            ],
        ),
        "output-loss" => (
            3,
            first,
            vec![output(first, 1, 0, Some(crate::SurfaceId::new(3, 3)))],
            vec![
                surface(3, 3, first),
                surface(4, 3, first),
                surface(5, 3, first),
            ],
        ),
        "returned-output-generation" => (
            4,
            second,
            vec![
                output(first, 1, 0, Some(crate::SurfaceId::new(3, 4))),
                output(second, 2, 1200, Some(crate::SurfaceId::new(5, 4))),
            ],
            vec![
                surface(3, 4, first),
                surface(4, 4, first),
                surface(5, 4, second),
            ],
        ),
        _ => return None,
    };
    Some(crate::PolicySceneSnapshot {
        generation,
        active_output,
        outputs,
        surfaces,
        session_operations: Vec::new(),
    })
}

fn output(
    output: crate::OutputId,
    generation: u64,
    x: i32,
    focus: Option<crate::SurfaceId>,
) -> crate::PolicyOutputSnapshot {
    crate::PolicyOutputSnapshot {
        output,
        generation,
        focus,
        bounds: crate::Rect {
            x,
            y: 0,
            width: 1200,
            height: 800,
        },
        work_area: crate::Rect {
            x,
            y: 0,
            width: 1200,
            height: 800,
        },
    }
}

fn surface(index: u32, generation: u32, output: crate::OutputId) -> crate::PolicySurfaceSnapshot {
    crate::PolicySurfaceSnapshot {
        surface: crate::SurfaceId::new(index, generation),
        generation: u64::from(generation),
        current_output: Some(output),
        kind: crate::PolicySurfaceKind::Toplevel,
        capabilities: crate::LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        constraints: crate::SurfaceConstraints {
            min_size: Some(crate::Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        exact_size: None,
        requested_state: crate::PolicyPresentationState::default(),
        current_state: crate::PolicyPresentationState::default(),
        transient_owner: None,
        geometry: crate::Rect {
            x: if output == crate::OutputId::from_raw(2) {
                1200
            } else {
                0
            },
            y: 0,
            width: 600,
            height: 800,
        },
    }
}
