use sophia_engine::CursorShape;
use sophia_renderer_live::{CursorThemeError, decode_xcursor_asset, resolve_cursor_theme};

/// One cursor frame: width, height, delay, hotspot, and its fill byte.
type CursorFrameSpec = (u32, u32, u32, (u32, u32), u8);

fn cursor_file(size: u32, width: u32, height: u32, hotspot: (u32, u32)) -> Vec<u8> {
    cursor_file_with_frames(&[(size, width, height, hotspot, 0x7f)])
}

fn cursor_file_with_frames(frames: &[CursorFrameSpec]) -> Vec<u8> {
    let toc_end = 16 + frames.len() * 12;
    let mut position = u32::try_from(toc_end).unwrap();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"Xcur");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&0x0001_0000_u32.to_le_bytes());
    bytes.extend_from_slice(&u32::try_from(frames.len()).unwrap().to_le_bytes());
    for (size, width, height, _, _) in frames {
        bytes.extend_from_slice(&0xfffd_0002_u32.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&position.to_le_bytes());
        position = position
            .checked_add(36 + width.saturating_mul(*height).saturating_mul(4))
            .unwrap();
    }
    for (size, width, height, hotspot, fill) in frames {
        bytes.extend_from_slice(&36_u32.to_le_bytes());
        bytes.extend_from_slice(&0xfffd_0002_u32.to_le_bytes());
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&hotspot.0.to_le_bytes());
        bytes.extend_from_slice(&hotspot.1.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.resize(bytes.len() + (width * height * 4) as usize, *fill);
    }
    bytes
}

#[test]
fn bounded_decoder_retains_pixels_and_hotspot() {
    let decoded = decode_xcursor_asset(&cursor_file(24, 2, 3, (1, 2)), 24, 7).unwrap();

    assert_eq!(decoded.nominal_size, 24);
    assert_eq!((decoded.asset.width(), decoded.asset.height()), (2, 3));
    assert_eq!(decoded.asset.hotspot(), (1, 2));
    assert_eq!(decoded.asset.generation(), 7);
    assert_eq!(decoded.asset.pixels(), &[0x7f; 24]);
}

#[test]
fn bounded_decoder_rejects_dimensions_before_allocating() {
    let error = decode_xcursor_asset(&cursor_file(16, 129, 1, (0, 0)), 16, 1).unwrap_err();
    assert!(matches!(error, CursorThemeError::InvalidAsset(_)));
}

#[test]
fn closest_size_and_first_animation_frame_win_deterministically() {
    let bytes = cursor_file_with_frames(&[
        (16, 1, 1, (0, 0), 0x22),
        (24, 1, 1, (0, 0), 0x44),
        (24, 1, 1, (0, 0), 0x66),
    ]);

    let decoded = decode_xcursor_asset(&bytes, 22, 1).unwrap();

    assert_eq!(decoded.nominal_size, 24);
    assert_eq!(decoded.asset.pixels(), &[0x44; 4]);
    assert_eq!(decoded.ignored_animation_frames, 1);
}

#[test]
fn missing_theme_falls_back_audibly_to_x11_core() {
    let resolution = resolve_cursor_theme(
        "sophia-theme-that-does-not-exist",
        24,
        CursorShape::LeftPtr,
        1,
    );

    assert_eq!(resolution.effective_theme, "x11-core");
    assert!(resolution.fallback_reason.is_some());
    assert_eq!(resolution.asset.hotspot(), (1, 1));
}
