use sophia_engine::{
    ChromeLayoutError, CompositorNodeId, FocusRingStyle, SurfaceChromeRole, SurfaceChromeStyle,
    SurfaceFrameStyle, apply_surface_chrome_clearance, compositor_border_bands,
    outer_surface_constraints, outer_surface_geometry, surface_chrome_display_list,
    surface_chrome_display_list_for_surfaces, surface_content_geometry,
};
use sophia_protocol::{
    BufferSource, CommittedSurfaceState, LayoutTransaction, OutputId, Rect, Region, Size,
    SurfaceConstraints, SurfaceId, SurfacePlacement, SurfaceSizeRequest, TransactionId, Transform,
};

#[test]
fn chrome_clearance_insets_wm_allocations_once_and_preserves_outer_extent() {
    let surface = SurfaceId::new(1, 1);
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: vec![SurfaceSizeRequest {
            surface,
            size: Size {
                width: 100,
                height: 80,
            },
        }],
        focus: Some(surface),
        render_positions: vec![SurfacePlacement {
            surface,
            geometry: Rect {
                x: 10,
                y: 20,
                width: 100,
                height: 80,
            },
            z_index: 0,
            crop: None,
            transform: Transform::IDENTITY,
        }],
        timeout_msec: 300,
    };
    let style = SurfaceChromeStyle {
        focus_ring: FocusRingStyle {
            width: 6,
            ..FocusRingStyle::default()
        },
        frame: SurfaceFrameStyle {
            width: 3,
            ..SurfaceFrameStyle::default()
        },
    };

    let content = apply_surface_chrome_clearance(&transaction, style).unwrap();
    assert_eq!(
        content.requested_sizes[0].size,
        Size {
            width: 88,
            height: 68,
        }
    );
    assert_eq!(
        content.render_positions[0].geometry,
        Rect {
            x: 16,
            y: 26,
            width: 88,
            height: 68,
        }
    );
    let geometry = content.render_positions[0].geometry;
    let committed = CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer { handle: 1 },
            sophia_protocol::Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),
        damage: Region::single(geometry),
    };
    let list = surface_chrome_display_list(
        OutputId::from_raw(1),
        &[surface],
        &[committed],
        Some(surface),
        style,
    )
    .unwrap();
    let ring = list
        .borders()
        .find(|border| {
            border.node
                == (CompositorNodeId::SurfaceChrome {
                    surface,
                    role: SurfaceChromeRole::FocusRing,
                })
        })
        .unwrap();
    assert_eq!(ring.outer, transaction.render_positions[0].geometry);
    assert!(
        compositor_border_bands(ring)
            .into_iter()
            .all(|band| !rects_overlap(band.geometry, ring.inner))
    );
}

#[test]
fn frame_applies_only_to_explicitly_eligible_managed_surfaces() {
    let first = SurfaceId::new(1, 1);
    let bar = SurfaceId::new(2, 1);
    let first_state = committed(
        first,
        Rect {
            x: 2,
            y: 2,
            width: 96,
            height: 76,
        },
    );
    let bar_state = committed(
        bar,
        Rect {
            x: 0,
            y: 80,
            width: 100,
            height: 20,
        },
    );
    let style = SurfaceChromeStyle {
        focus_ring: FocusRingStyle {
            width: 0,
            ..FocusRingStyle::default()
        },
        frame: SurfaceFrameStyle {
            width: 2,
            ..SurfaceFrameStyle::default()
        },
    };
    let list = surface_chrome_display_list_for_surfaces(
        OutputId::from_raw(1),
        &[first, bar],
        &[first],
        &[first_state, bar_state],
        Some(first),
        style,
    )
    .unwrap();

    let borders = list.borders().collect::<Vec<_>>();
    assert_eq!(borders.len(), 1);
    assert_eq!(
        borders[0].node,
        CompositorNodeId::SurfaceChrome {
            surface: first,
            role: SurfaceChromeRole::Frame,
        }
    );
}

#[test]
fn clearance_rejects_allocations_that_cannot_preserve_client_content() {
    let surface = SurfaceId::new(1, 1);
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(1),
        requested_sizes: vec![SurfaceSizeRequest {
            surface,
            size: Size {
                width: 10,
                height: 10,
            },
        }],
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    };
    let style = SurfaceChromeStyle {
        focus_ring: FocusRingStyle {
            width: 6,
            ..FocusRingStyle::default()
        },
        ..SurfaceChromeStyle::default()
    };

    assert_eq!(
        apply_surface_chrome_clearance(&transaction, style),
        Err(ChromeLayoutError::AllocationTooSmall)
    );
}

#[test]
fn content_constraints_round_trip_through_outer_chrome_allocation() {
    let style = SurfaceChromeStyle {
        frame: SurfaceFrameStyle {
            width: 2,
            ..SurfaceFrameStyle::default()
        },
        ..SurfaceChromeStyle::default()
    };
    let content = Size {
        width: 500,
        height: 500,
    };
    let outer = outer_surface_constraints(
        SurfaceConstraints {
            min_size: Some(content),
            max_size: Some(content),
        },
        style,
    )
    .unwrap();

    assert_eq!(
        outer,
        SurfaceConstraints {
            min_size: Some(Size {
                width: 504,
                height: 504,
            }),
            max_size: Some(Size {
                width: 504,
                height: 504,
            }),
        }
    );
    let transaction = LayoutTransaction {
        transaction: TransactionId::from_raw(2),
        requested_sizes: vec![SurfaceSizeRequest {
            surface: SurfaceId::new(2, 1),
            size: outer.min_size.unwrap(),
        }],
        focus: None,
        render_positions: Vec::new(),
        timeout_msec: 300,
    };
    assert_eq!(
        apply_surface_chrome_clearance(&transaction, style)
            .unwrap()
            .requested_sizes[0]
            .size,
        content
    );
    assert_eq!(
        outer_surface_geometry(
            Rect {
                x: 2,
                y: 2,
                width: 500,
                height: 500,
            },
            style,
        )
        .unwrap(),
        Rect {
            x: 0,
            y: 0,
            width: 504,
            height: 504,
        }
    );
}

fn committed(surface: SurfaceId, geometry: Rect) -> CommittedSurfaceState {
    CommittedSurfaceState {
        surface,
        committed_generation: 1,
        geometry,
        content: sophia_protocol::SurfaceContentSet::singleton(
            BufferSource::CpuBuffer {
                handle: u64::from(surface.index()),
            },
            Size {
                width: geometry.width,
                height: geometry.height,
            },
        ),
        damage: Region::single(geometry),
    }
}

fn rects_overlap(first: Rect, second: Rect) -> bool {
    first.x < second.x.saturating_add(second.width)
        && second.x < first.x.saturating_add(first.width)
        && first.y < second.y.saturating_add(second.height)
        && second.y < first.y.saturating_add(first.height)
}

/// A surface given a whole allocation must leave room for its own chrome.
///
/// Chrome is drawn outside the content rect, so content that fills its
/// allocation puts the chrome outside the allocation -- and when the allocation
/// is a whole output, outside the output, where nothing can show it. The
/// conversion is what reserves that room, and the property worth pinning is that
/// the chrome lands back exactly on the allocation rather than anywhere near it.
#[test]
fn content_inside_an_allocation_leaves_its_chrome_exactly_on_the_allocation() {
    let style = SurfaceChromeStyle {
        focus_ring: FocusRingStyle {
            width: 2,
            ..FocusRingStyle::default()
        },
        ..SurfaceChromeStyle::default()
    };
    assert_eq!(style.clearance(), 2);

    // The case from the physical rig: one window allocated an entire output.
    let allocation = Rect {
        x: 0,
        y: 0,
        width: 1_920,
        height: 1_080,
    };
    let content = surface_content_geometry(allocation, style).unwrap();
    assert_eq!(
        content,
        Rect {
            x: 2,
            y: 2,
            width: 1_916,
            height: 1_076
        }
    );
    // Outsetting the content by the same clearance is the ring, and it lands on
    // the allocation to the pixel -- so nothing is drawn outside the output.
    assert_eq!(outer_surface_geometry(content, style).unwrap(), allocation);

    // Assigning the allocation as content instead is the defect: the ring then
    // starts two pixels above and left of the output it belongs to.
    let unconverted = outer_surface_geometry(allocation, style).unwrap();
    assert_eq!(unconverted.x, -2);
    assert_eq!(unconverted.y, -2);
    assert!(
        unconverted.x < allocation.x && unconverted.y < allocation.y,
        "an unconverted allocation puts its chrome outside the output"
    );
}

/// The conversion is an exact inverse in both directions, at every clearance.
#[test]
fn allocation_and_content_geometry_round_trip_at_every_clearance() {
    for width in [0, 1, 2, 7] {
        let style = SurfaceChromeStyle {
            focus_ring: FocusRingStyle {
                width,
                ..FocusRingStyle::default()
            },
            ..SurfaceChromeStyle::default()
        };
        let allocation = Rect {
            x: 100,
            y: 40,
            width: 800,
            height: 600,
        };
        let content = surface_content_geometry(allocation, style).unwrap();
        assert_eq!(outer_surface_geometry(content, style).unwrap(), allocation);
        assert_eq!(content.x, allocation.x + width);
        assert_eq!(content.width, allocation.width - 2 * width);
    }

    // An allocation too small to hold its own chrome is refused rather than
    // silently inverted into a negative extent.
    let style = SurfaceChromeStyle {
        focus_ring: FocusRingStyle {
            width: 40,
            ..FocusRingStyle::default()
        },
        ..SurfaceChromeStyle::default()
    };
    assert_eq!(
        surface_content_geometry(
            Rect {
                x: 0,
                y: 0,
                width: 60,
                height: 60
            },
            style
        ),
        Err(ChromeLayoutError::AllocationTooSmall)
    );
}
