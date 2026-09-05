#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use std::collections::BTreeMap;

use sophia_backend_live::live_surfaces_owned_by_output;
use sophia_protocol::{OutputId, SurfaceId};

/// A head composites its own display's windows and no others.
///
/// The case that made this necessary: DP-1 at x=0 is 2560 wide, DP-2 sits at
/// x=2560, and a scroller places its third column at x=2552. That column is
/// past DP-1's right edge, which in a scroller is ordinary -- the strip is
/// longer than the display and the camera has not been scrolled to it. It is
/// also inside DP-2's rectangle, so selecting by geometry drew DP-1's window
/// on DP-2.
#[test]
fn a_head_composites_only_the_surfaces_placed_on_it() {
    let first = OutputId::from_raw(1);
    let second = OutputId::from_raw(2);
    let (left, middle, offscreen) = (
        SurfaceId::new(1, 1),
        SurfaceId::new(2, 1),
        SurfaceId::new(3, 1),
    );
    let order = vec![left, middle, offscreen];
    let owners = BTreeMap::from([(left, first), (middle, first), (offscreen, first)]);

    assert_eq!(
        live_surfaces_owned_by_output(&order, &owners, first),
        order,
        "every column of this strip belongs to the display it was placed on, \
         including the one scrolled past its edge"
    );
    assert!(
        live_surfaces_owned_by_output(&order, &owners, second).is_empty(),
        "the display beside it draws none of them, whatever their coordinates"
    );
}

/// Order is preserved, because it is stacking order.
#[test]
fn ownership_filtering_keeps_the_presentation_order() {
    let first = OutputId::from_raw(1);
    let second = OutputId::from_raw(2);
    let (a, b, c, d) = (
        SurfaceId::new(1, 1),
        SurfaceId::new(2, 1),
        SurfaceId::new(3, 1),
        SurfaceId::new(4, 1),
    );
    let order = vec![a, b, c, d];
    let owners = BTreeMap::from([(a, first), (b, second), (c, first), (d, second)]);

    assert_eq!(
        live_surfaces_owned_by_output(&order, &owners, first),
        vec![a, c]
    );
    assert_eq!(
        live_surfaces_owned_by_output(&order, &owners, second),
        vec![b, d]
    );
}

/// A surface no projection has placed belongs to no head.
///
/// Nothing is presented before its first placement -- the admission path
/// places, then arms, then presents -- so this is unobservable rather than a
/// window that never appears.
#[test]
fn an_unplaced_surface_belongs_to_no_head() {
    let first = OutputId::from_raw(1);
    let second = OutputId::from_raw(2);
    let unplaced = SurfaceId::new(9, 1);
    let order = vec![unplaced];
    let owners = BTreeMap::new();

    assert!(live_surfaces_owned_by_output(&order, &owners, first).is_empty());
    assert!(live_surfaces_owned_by_output(&order, &owners, second).is_empty());
}
