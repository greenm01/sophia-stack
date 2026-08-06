#![cfg(all(feature = "libdrm-events", feature = "gbm-probe"))]

use sophia_backend_live::{
    LiveRendererImageHandoffAdmission, reduce_live_renderer_image_handoff_admission,
};
use sophia_renderer_live::LiveRendererImageId;

fn image(raw: u64) -> LiveRendererImageId {
    LiveRendererImageId::from_raw(raw)
}

#[test]
fn renderer_handoff_requires_exact_unique_retained_image_coverage() {
    use LiveRendererImageHandoffAdmission::{
        CoverageMismatch, DuplicateIdentity, InvalidIdentity, Missing, Ready,
    };

    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[], None),
        Ready
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(
            &[image(2), image(1)],
            Some(&[image(1), image(2)]),
        ),
        Ready
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(1)], None),
        Missing
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(1), image(2)], Some(&[image(1)]),),
        CoverageMismatch
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(1)], Some(&[image(1), image(2)]),),
        CoverageMismatch
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(1), image(1)], Some(&[image(1)]),),
        DuplicateIdentity
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(1)], Some(&[image(1), image(1)]),),
        DuplicateIdentity
    );
    assert_eq!(
        reduce_live_renderer_image_handoff_admission(&[image(0)], Some(&[image(0)])),
        InvalidIdentity
    );
}
