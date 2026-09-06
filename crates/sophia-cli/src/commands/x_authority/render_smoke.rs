/// Drive RENDER from a real client and read the blended pixels back.
///
/// The proof is a round trip rather than acceptance: every stage composites
/// values chosen so the correct result is one specific byte pattern, then
/// reads the drawable through `GetImage` and checks it. A server that
/// accepted every request and drew nothing would pass an acceptance test and
/// fails this one.
fn run_x_authority_render_smoke()
-> Result<XAuthorityRenderSmokeReport, Box<dyn std::error::Error>> {
    use x11rb::connection::Connection;
    use x11rb::protocol::render::{
        ConnectionExt as _, CreatePictureAux, Glyphinfo, PictOp, Repeat,
    };
    use x11rb::protocol::xproto::ConnectionExt as _;

    let display_number = 700 + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once(&server_path, NamespaceId::from_raw(52))
    });

    wait_for_socket_path(&socket_path)?;
    let (connection, screen_index) = x11rb::connect(Some(&display))?;
    let screen = &connection.setup().roots[screen_index];
    let root = screen.root;

    let version = connection.render_query_version(0, 11)?.reply()?;
    let formats = connection.render_query_pict_formats()?.reply()?;
    // A toolkit needs a premultiplied 32-bit format for its buffers and an
    // 8-bit alpha format for glyph coverage; without both it falls back.
    let argb32 = formats
        .formats
        .iter()
        .find(|format| format.depth == 32 && format.direct.alpha_mask == 0xff)
        .ok_or("the server reported no ARGB32 picture format")?
        .id;
    let a8 = formats
        .formats
        .iter()
        .find(|format| format.depth == 8 && format.direct.alpha_mask == 0xff)
        .ok_or("the server reported no A8 picture format")?
        .id;

    // A one-pixel repeating source, which is how a client paints a solid
    // colour before CreateSolidFill exists.
    let source_pixmap = connection.generate_id()?;
    connection.create_pixmap(32, source_pixmap, root, 1, 1)?;
    let source = connection.generate_id()?;
    connection.render_create_picture(
        source,
        source_pixmap,
        argb32,
        &CreatePictureAux::new().repeat(Repeat::NORMAL),
    )?;
    // Opaque red, premultiplied.
    connection.render_fill_rectangles(
        PictOp::SRC,
        source,
        x11rb::protocol::render::Color {
            red: 0xffff,
            green: 0,
            blue: 0,
            alpha: 0xffff,
        },
        &[x11rb::protocol::xproto::Rectangle {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        }],
    )?;

    // A destination filled with half-alpha blue, so the Over below has
    // something to blend against rather than transparent black.
    let destination_pixmap = connection.generate_id()?;
    connection.create_pixmap(32, destination_pixmap, root, 4, 4)?;
    let destination = connection.generate_id()?;
    connection.render_create_picture(
        destination,
        destination_pixmap,
        argb32,
        &CreatePictureAux::new(),
    )?;
    connection.render_fill_rectangles(
        PictOp::SRC,
        destination,
        x11rb::protocol::render::Color {
            red: 0,
            green: 0,
            blue: 0x8080,
            alpha: 0x8080,
        },
        &[x11rb::protocol::xproto::Rectangle {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }],
    )?;

    // Composite the repeating red over it. Over with an opaque source
    // replaces the destination entirely, so every pixel must read as red.
    connection.render_composite(
        PictOp::OVER,
        source,
        x11rb::protocol::render::Picture::default(),
        destination,
        0,
        0,
        0,
        0,
        0,
        0,
        4,
        4,
    )?;
    connection.flush()?;
    let image = connection
        .get_image(
            x11rb::protocol::xproto::ImageFormat::Z_PIXMAP,
            destination_pixmap,
            1,
            1,
            1,
            1,
            u32::MAX,
        )?
        .reply()?;
    let composited_pixel = [image.data[0], image.data[1], image.data[2], image.data[3]];

    // Glyphs: a 2x1 A8 glyph, fully covered then half covered, composited
    // with the red source. The half-covered pixel is the antialiasing proof.
    let glyphset = connection.generate_id()?;
    connection.render_create_glyph_set(glyphset, a8)?;
    connection.render_add_glyphs(
        glyphset,
        &[7],
        &[Glyphinfo {
            width: 2,
            height: 1,
            x: 0,
            y: 0,
            x_off: 2,
            y_off: 0,
        }],
        // A8 scanlines pad to four bytes.
        &[0xff, 0x80, 0, 0],
    )?;
    let glyph_target_pixmap = connection.generate_id()?;
    connection.create_pixmap(32, glyph_target_pixmap, root, 4, 4)?;
    let glyph_target = connection.generate_id()?;
    connection.render_create_picture(
        glyph_target,
        glyph_target_pixmap,
        argb32,
        &CreatePictureAux::new(),
    )?;
    // One element: a run of one glyph at offset (0, 0).
    let mut glyph_elements = vec![1u8, 0, 0, 0];
    glyph_elements.extend_from_slice(&0i16.to_ne_bytes());
    glyph_elements.extend_from_slice(&0i16.to_ne_bytes());
    glyph_elements.extend_from_slice(&[7, 0, 0, 0]);
    connection.render_composite_glyphs8(
        PictOp::OVER,
        source,
        glyph_target,
        a8,
        glyphset,
        0,
        0,
        &glyph_elements,
    )?;
    connection.flush()?;
    let glyph_image = connection
        .get_image(
            x11rb::protocol::xproto::ImageFormat::Z_PIXMAP,
            glyph_target_pixmap,
            1,
            0,
            1,
            1,
            u32::MAX,
        )?
        .reply()?;
    let glyph_pixel = [
        glyph_image.data[0],
        glyph_image.data[1],
        glyph_image.data[2],
        glyph_image.data[3],
    ];

    // A cursor from a picture, the shape libXcursor sends. The source is
    // already premultiplied, which is what the engine's asset contract wants.
    let cursor_pixmap = connection.generate_id()?;
    connection.create_pixmap(32, cursor_pixmap, root, 8, 8)?;
    let cursor_picture = connection.generate_id()?;
    connection.render_create_picture(
        cursor_picture,
        cursor_pixmap,
        argb32,
        &CreatePictureAux::new(),
    )?;
    connection.render_fill_rectangles(
        PictOp::SRC,
        cursor_picture,
        x11rb::protocol::render::Color {
            red: 0x8080,
            green: 0,
            blue: 0,
            alpha: 0x8080,
        },
        &[x11rb::protocol::xproto::Rectangle {
            x: 0,
            y: 0,
            width: 8,
            height: 8,
        }],
    )?;
    let cursor = connection.generate_id()?;
    connection.render_create_cursor(cursor, cursor_picture, 1, 1)?;
    connection.free_cursor(cursor)?;
    connection.flush()?;

    let mut errors = 0usize;
    while let Some(event) = connection.poll_for_event()? {
        if matches!(event, Event::Error(_)) {
            errors += 1;
        }
    }

    // Opaque red over anything is opaque red.
    if composited_pixel != [0, 0, 0xff, 0xff] {
        return Err(format!(
            "composited pixel was {composited_pixel:?}, expected [0, 0, 255, 255]"
        )
        .into());
    }
    // Half coverage scales the premultiplied red: 0xff * 0x80 / 255 = 0x80.
    if glyph_pixel != [0, 0, 0x80, 0x80] {
        return Err(format!(
            "half-covered glyph pixel was {glyph_pixel:?}, expected [0, 0, 128, 128]"
        )
        .into());
    }

    drop(connection);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")??;

    Ok(XAuthorityRenderSmokeReport {
        display,
        major_version: version.major_version,
        minor_version: version.minor_version,
        formats: formats.formats.len(),
        composited_pixel,
        glyph_pixel,
        errors,
    })
}
