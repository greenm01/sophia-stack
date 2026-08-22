const WM_SESSION: &str = include_str!("../src/commands/live_session/wm/session.rs");
const PUBLIC_POLICY: &str = include_str!("../src/commands/live_session/wm/public_policy.rs");

fn offset(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("missing {needle:?}"))
}

/// A work area that moved must be relaid out whoever is driving policy.
///
/// The relayout check sat below the early return into the public path, so it ran
/// only for a private policy. Three places set `work_area_relayout_required` and
/// one place read it, and that reader was unreachable in every session running a
/// public policy client -- which is every session that runs the reference WM.
///
/// The visible cost was window chrome. Chrome clearance changing from zero to
/// two raised the flag, nothing consumed it, and windows stayed placed against
/// the old clearance while their focus ring was drawn against the new one. The
/// ring is drawn outside the window geometry, so it landed outside the output
/// entirely, and the only part visible anywhere was the sliver that crossed into
/// a neighbouring output in root space -- a border from one monitor's window
/// appearing on another monitor, and no border at all on its own.
///
/// `enqueue_relayout` opens by handling the public case, so the capability was
/// always there; only the ordering kept it from being reached.
#[test]
fn a_moved_work_area_is_relaid_out_for_public_and_private_policy_alike() {
    let relayout = offset(&WM_SESSION[..], "if self.work_area_relayout_required {");
    let public_return = offset(
        &WM_SESSION[..],
        "return self.poll_public_request(layout, output, allow_new_cycle);",
    );
    assert!(
        relayout < public_return,
        "the relayout check must precede the public early return, or a public \
         policy never sees a work-area change"
    );

    // The reader is still the only one, so its position is the whole guarantee.
    assert_eq!(
        WM_SESSION
            .matches("if self.work_area_relayout_required {")
            .count(),
        1,
        "a second reader would make the ordering above insufficient"
    );
    // And the capability it reaches genuinely covers the public case.
    let enqueue = offset(&WM_SESSION[..], "fn enqueue_relayout(");
    let public_branch = WM_SESSION[enqueue..]
        .find("if let Some(public) = self.public.as_mut() {")
        .expect("enqueue_relayout handles the public policy case");
    let private_work = WM_SESSION[enqueue..]
        .find("if self.has_current_relayout_request(layout) {")
        .expect("enqueue_relayout also handles the private case");
    assert!(
        public_branch < private_work,
        "enqueue_relayout answers the public case before falling through"
    );

    // Nothing in the public path consumes the flag itself, which is why the
    // ordering above is what makes it reachable at all.
    assert!(
        !PUBLIC_POLICY.contains("work_area_relayout_required = true"),
        "the public path does not raise this flag; it is raised by chrome, \
         commit, and work-area changes and consumed in one place"
    );
}

const PUBLIC_PROPOSAL: &str =
    include_str!("../src/commands/live_session/wm/public_policy/proposal.rs");

/// A public policy's placement is an allocation, not client geometry.
///
/// The public API separates an outer allocation from an optional content-size
/// request -- the code says so in a comment three lines above where it used to
/// assign the allocation straight into the layer as geometry. The private path
/// has always converted, through `apply_surface_chrome_clearance`. The public
/// path did not, so every surface it placed occupied its whole allocation and
/// the chrome drawn around that had nowhere to go: a focused window filling an
/// output put its focus ring wholly outside that output, and the only part
/// anyone saw was the sliver crossing into a neighbouring one.
#[test]
fn a_public_placement_is_converted_to_content_geometry_before_it_becomes_a_layer() {
    assert!(
        PUBLIC_PROPOSAL.contains("sophia_engine::surface_content_geometry(placement.geometry"),
        "the public path must convert an allocation into content geometry"
    );
    assert!(
        !PUBLIC_PROPOSAL.contains("layer.geometry = placement.geometry;"),
        "assigning the allocation as geometry is the defect this replaced"
    );
}

/// Converting the geometry without requesting the size is half a change.
///
/// A public peer omits a content-size request when it believes the client's
/// content need not move. Chrome clearance changing under a stable allocation
/// moves it anyway, and the reconciler adds requests only where the peer already
/// made one, so nothing else in this path would tell the client. Shipping the
/// conversion alone shrank every surface and left the layout waiting on an
/// acknowledgement that could not arrive: the transaction timed out once per
/// cycle and the session ended with the work still pending.
#[test]
fn a_content_extent_that_moved_carries_a_size_request_with_it() {
    let convert = PUBLIC_PROPOSAL
        .find("sophia_engine::surface_content_geometry(placement.geometry")
        .expect("the public path converts the allocation");
    let request = PUBLIC_PROPOSAL[convert..]
        .find("requested_sizes.insert(")
        .expect("a converted extent is accompanied by a size request");
    let push = PUBLIC_PROPOSAL[convert..]
        .find("layers.push(layer);")
        .expect("the layer is pushed after it is built");
    assert!(
        request < push,
        "the size request must be decided before the layer is committed"
    );
    assert!(
        PUBLIC_PROPOSAL.contains("layer.geometry.width != previous.width"),
        "the request is raised by comparing the converted extent with the one \
         the surface already had, not by assuming every placement changed it"
    );
}
