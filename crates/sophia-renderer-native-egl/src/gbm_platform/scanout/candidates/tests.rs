#![cfg(test)]

use std::fs::File;
use std::os::fd::AsFd;

use super::{NativeDmaBufFrame, is_supported_rendered_scanout_candidate_shape};

#[test]
fn validates_bounded_linear_xrgb_descriptor() {
    let file = File::open("/dev/null").unwrap();
    let valid = NativeDmaBufFrame {
        width: 64,
        height: 32,
        format: 0x3432_5258,
        modifier: 0,
        fd: file.as_fd(),
        offset: 0,
        stride: 256,
    };
    assert!(valid.is_valid());
    assert!(
        !NativeDmaBufFrame {
            stride: 64,
            ..valid
        }
        .is_valid()
    );
}

#[test]
fn rendered_scanout_candidate_shape_requires_single_plane() {
    assert!(is_supported_rendered_scanout_candidate_shape(1));
    assert!(!is_supported_rendered_scanout_candidate_shape(0));
    assert!(!is_supported_rendered_scanout_candidate_shape(2));
    assert!(!is_supported_rendered_scanout_candidate_shape(4));
}
