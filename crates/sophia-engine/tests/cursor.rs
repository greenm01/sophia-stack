use sophia_engine::{CursorAsset, CursorAssetError, CursorShape, x11_core_left_ptr_cursor};

#[test]
fn cursor_asset_validates_dimensions_hotspot_and_length() {
    assert_eq!(
        CursorAsset::new(0, 1, 0, 0, 1, Vec::new()),
        Err(CursorAssetError::InvalidDimensions)
    );
    assert_eq!(
        CursorAsset::new(1, 1, 1, 0, 1, vec![0; 4]),
        Err(CursorAssetError::InvalidHotspot)
    );
    assert_eq!(
        CursorAsset::new(1, 1, 0, 0, 1, vec![0; 3]),
        Err(CursorAssetError::InvalidPixelLength {
            expected: 4,
            actual: 3,
        })
    );
}

#[test]
fn x11_left_ptr_is_stable_and_uses_its_canonical_hotspot() {
    let first = x11_core_left_ptr_cursor(1);
    let next_generation = x11_core_left_ptr_cursor(2);

    assert_eq!((first.width(), first.height()), (10, 16));
    assert_eq!(first.hotspot(), (1, 1));
    assert_eq!(first.digest(), next_generation.digest());
    assert_eq!(first.pixels().len(), 10 * 16 * 4);
    assert!(
        first
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel == [0, 0, 0, 0xff])
    );
    assert!(
        first
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel == [0xff, 0xff, 0xff, 0xff])
    );
}

#[test]
fn cursor_shape_aliases_reduce_to_semantic_roles() {
    assert_eq!(CursorShape::parse("default"), Some(CursorShape::LeftPtr));
    assert_eq!(CursorShape::parse("xterm"), Some(CursorShape::Text));
    assert_eq!(CursorShape::parse("hand2"), Some(CursorShape::Pointer));
    assert_eq!(CursorShape::parse("private-renderer-role"), None);
}
