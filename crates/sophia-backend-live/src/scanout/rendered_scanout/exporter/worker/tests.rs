#![cfg(test)]

use super::reusable_cpu_buffer_damage;
use sophia_protocol::Size;

#[test]
fn changed_cpu_buffer_without_snapshot_repaints_the_full_output() {
    let size = Size {
        width: 3840,
        height: 960,
    };
    let damage = reusable_cpu_buffer_damage(1, None, 2, None, size);

    assert_eq!(damage.len(), 1);
    assert_eq!(damage[0].width, size.width);
    assert_eq!(damage[0].height, size.height);
}

#[test]
fn unchanged_cpu_buffer_requires_no_rewrite() {
    let damage = reusable_cpu_buffer_damage(
        7,
        None,
        7,
        None,
        Size {
            width: 1920,
            height: 1080,
        },
    );

    assert!(damage.is_empty());
}
