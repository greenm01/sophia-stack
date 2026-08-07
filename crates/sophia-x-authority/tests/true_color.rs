use sophia_x_authority::{
    X_SETUP_ARGB_VISUAL, X_SETUP_DEFAULT_VISUAL, XColorRgb16, x_lookup_color_name,
    x_true_color_visual,
};

#[test]
fn advertised_true_color_visuals_share_rgb_masks_and_keep_argb_alpha() {
    let xrgb = x_true_color_visual(X_SETUP_DEFAULT_VISUAL).unwrap();
    let argb = x_true_color_visual(X_SETUP_ARGB_VISUAL).unwrap();

    assert_eq!(xrgb.depth, 24);
    assert_eq!(xrgb.valid_pixel_mask(), 0x00ff_ffff);
    assert_eq!(argb.depth, 32);
    assert_eq!(argb.valid_pixel_mask(), u32::MAX);
    assert_eq!(argb.alpha_mask, 0xff00_0000);
    assert!(x_true_color_visual(0xdead_beef).is_none());
}

#[test]
fn true_color_allocation_uses_x11_high_byte_quantization() {
    let visual = x_true_color_visual(X_SETUP_DEFAULT_VISUAL).unwrap();
    let screen = visual.screen_color(XColorRgb16 {
        red: 0x1234,
        green: 0xabcd,
        blue: 0x80ff,
    });

    assert_eq!(
        screen,
        XColorRgb16 {
            red: 0x1212,
            green: 0xabab,
            blue: 0x8080,
        }
    );
    assert_eq!(visual.pixel(screen), 0x0012_ab80);
    assert_eq!(visual.query(0x0012_ab80), Some(screen));
    assert!(visual.query(0x0112_ab80).is_none());
}

#[test]
fn argb_allocations_are_opaque_and_query_ignores_alpha() {
    let visual = x_true_color_visual(X_SETUP_ARGB_VISUAL).unwrap();
    let screen = XColorRgb16 {
        red: 0x1212,
        green: 0xabab,
        blue: 0x8080,
    };

    assert_eq!(visual.pixel(screen), 0xff12_ab80);
    assert_eq!(visual.query(0x7f12_ab80), Some(screen));
}

#[test]
fn retained_color_names_are_bounded_and_deterministic() {
    assert_eq!(
        x_lookup_color_name("Light Gray"),
        Some(XColorRgb16 {
            red: 0xd3d3,
            green: 0xd3d3,
            blue: 0xd3d3,
        })
    );
    assert_eq!(
        x_lookup_color_name("gray50"),
        Some(XColorRgb16 {
            red: 0x7f7f,
            green: 0x7f7f,
            blue: 0x7f7f,
        })
    );
    assert!(x_lookup_color_name("not-a-retained-color").is_none());
}
