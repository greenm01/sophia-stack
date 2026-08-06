#![cfg(test)]

use super::*;

fn image(raw: u64) -> LiveRendererImageId {
    LiveRendererImageId::from_raw(raw)
}

#[test]
fn native_resume_requires_the_exact_retained_renderer_generation() {
    assert_eq!(validate_renderer_image_resume_admission(&[], None), Ok(()));
    assert_eq!(
        validate_renderer_image_resume_admission(
            &[image(4), image(9)],
            Some(&[image(4), image(9)])
        ),
        Ok(())
    );
    assert!(validate_renderer_image_resume_admission(&[image(4)], None).is_err());
    assert!(validate_renderer_image_resume_admission(&[image(4)], Some(&[image(9)])).is_err());
    assert!(validate_renderer_image_resume_admission(&[], Some(&[image(4)])).is_err());
}
