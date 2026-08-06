#![cfg(test)]

use super::*;

fn image(raw: u64) -> sophia_renderer_live::LiveRendererImageId {
    sophia_renderer_live::LiveRendererImageId::from_raw(raw)
}

#[test]
fn renderer_handoff_requires_exact_unique_retained_image_coverage() {
    assert_eq!(
        validate_renderer_image_handoff_ids(&[image(2), image(1)], &[image(1), image(2)]),
        Ok(())
    );
    assert!(validate_renderer_image_handoff_ids(&[image(1), image(2)], &[image(1)]).is_err());
    assert!(validate_renderer_image_handoff_ids(&[image(1)], &[image(1), image(2)]).is_err());
    assert!(validate_renderer_image_handoff_ids(&[image(1), image(1)], &[image(1)]).is_err());
    assert!(validate_renderer_image_handoff_ids(&[image(0)], &[image(0)]).is_err());
}
