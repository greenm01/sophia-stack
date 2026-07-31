#![cfg(test)]

use super::*;

#[test]
fn damage_copy_updates_only_clipped_rectangles() {
    let source = (0_u8..64).collect::<Vec<_>>();
    let mut target = vec![0_u8; 80];
    copy_xrgb8888_damage(
        &source,
        16,
        4,
        4,
        &mut target,
        20,
        &[
            NativeCompositionRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
            NativeCompositionRect {
                x: 3,
                y: 3,
                width: 4,
                height: 4,
            },
        ],
    )
    .unwrap();

    assert_eq!(&target[24..32], &source[20..28]);
    assert_eq!(&target[44..52], &source[36..44]);
    assert_eq!(&target[72..76], &source[60..64]);
    assert!(target[..24].iter().all(|byte| *byte == 0));
    assert!(target[32..44].iter().all(|byte| *byte == 0));
}
