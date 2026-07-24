use sophia_protocol::{CURSOR_IMAGE_MAX_EDGE, CursorSnapshot, Point, Size};

#[test]
fn rejects_oversized_or_truncated_images() {
    let mut snapshot = CursorSnapshot {
        visible: true,
        position: Point::default(),
        hotspot: Point::default(),
        image_size: Size {
            width: 16,
            height: 16,
        },
        argb8888: vec![0; 16 * 16 * 4],
        generation: 1,
    };
    assert!(snapshot.image_is_valid());
    snapshot.argb8888.pop();
    assert!(!snapshot.image_is_valid());
    snapshot.image_size.width = CURSOR_IMAGE_MAX_EDGE + 1;
    assert!(!snapshot.image_is_valid());
}
