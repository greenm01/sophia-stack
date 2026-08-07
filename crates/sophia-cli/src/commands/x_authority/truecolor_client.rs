use std::io::{Error, ErrorKind};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, ImageOrder,
    PropMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;

const WIDTH: u16 = 640;
const HEIGHT: u16 = 240;
const PUT_IMAGE_ROWS: u16 = 60;
const BARS: [(u16, u32); 7] = [
    (40, 0x00ff_0000),
    (60, 0x0000_ff00),
    (80, 0x0000_00ff),
    (100, 0x00ff_ff00),
    (120, 0x0000_ffff),
    (140, 0x00ff_00ff),
    (100, 0x0080_8080),
];

pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let display = std::env::var("DISPLAY")
        .map_err(|_| Error::new(ErrorKind::NotFound, "DISPLAY is not set"))?;
    let (connection, screen_number) = x11rb::connect(Some(&display))?;
    let setup = connection.setup();
    let screen = setup
        .roots
        .get(screen_number)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "DISPLAY names an unknown screen"))?;
    if screen.root_depth != 24 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "TrueColor proof requires root depth 24, got {}",
                screen.root_depth
            ),
        )
        .into());
    }
    let format = setup
        .pixmap_formats
        .iter()
        .find(|format| format.depth == 24)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "depth-24 pixmap format is missing"))?;
    if format.bits_per_pixel != 32 || format.scanline_pad != 32 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "TrueColor proof requires depth-24 bpp-32 scanlines, got bpp={} pad={}",
                format.bits_per_pixel, format.scanline_pad
            ),
        )
        .into());
    }
    if screen.width_in_pixels < 3440 || screen.height_in_pixels < 304 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "TrueColor proof requires the retained two-output root geometry",
        )
        .into());
    }

    verify_colormap(&connection, screen.default_colormap)?;

    let window = connection.generate_id()?;
    let gc = connection.generate_id()?;
    connection.create_window(
        screen.root_depth,
        window,
        screen.root,
        2800,
        64,
        WIDTH,
        HEIGHT,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .override_redirect(1)
            .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY),
    )?;
    connection.change_property8(
        PropMode::REPLACE,
        window,
        AtomEnum::WM_NAME,
        AtomEnum::STRING,
        b"Sophia TrueColor Palette",
    )?;
    connection.create_gc(gc, window, &CreateGCAux::new())?;
    connection.map_window(window)?;
    connection.flush()?;
    wait_for_map(&connection, window)?;

    let pixels = palette_bytes(setup.image_byte_order);
    draw_palette(&connection, window, gc, &pixels)?;
    let readback = connection
        .get_image(ImageFormat::Z_PIXMAP, window, 0, 0, WIDTH, HEIGHT, u32::MAX)?
        .reply()?;
    if readback.depth != 24 || readback.data != pixels {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "TrueColor PutImage/GetImage bytes did not round-trip exactly",
        )
        .into());
    }

    println!(
        "sophia_truecolor_client schema=1 status=ready width={WIDTH} height={HEIGHT} palette=asymmetric_rgb_cmy_gray put_image=exact get_image=exact alloc_color=exact alloc_named_color=exact query_colors=exact"
    );
    connection.flush()?;

    loop {
        match connection.wait_for_event()? {
            Event::Expose(event) if event.window == window && event.count == 0 => {
                draw_palette(&connection, window, gc, &pixels)?;
            }
            Event::DestroyNotify(event) if event.window == window => return Ok(()),
            _ => {}
        }
    }
}

fn verify_colormap<C: Connection>(
    connection: &C,
    colormap: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let red = connection.alloc_color(colormap, u16::MAX, 0, 0)?.reply()?;
    if (red.red, red.green, red.blue, red.pixel) != (u16::MAX, 0, 0, 0x00ff_0000) {
        return Err(Error::new(ErrorKind::InvalidData, "AllocColor changed pure red").into());
    }
    let magenta = connection
        .alloc_named_color(colormap, b"magenta")?
        .reply()?;
    if (
        magenta.exact_red,
        magenta.exact_green,
        magenta.exact_blue,
        magenta.visual_red,
        magenta.visual_green,
        magenta.visual_blue,
        magenta.pixel,
    ) != (u16::MAX, 0, u16::MAX, u16::MAX, 0, u16::MAX, 0x00ff_00ff)
    {
        return Err(Error::new(ErrorKind::InvalidData, "AllocNamedColor changed magenta").into());
    }
    let colors = connection
        .query_colors(colormap, &[red.pixel, magenta.pixel])?
        .reply()?;
    let expected = [(u16::MAX, 0, 0), (u16::MAX, 0, u16::MAX)];
    if colors.colors.len() != expected.len()
        || colors
            .colors
            .iter()
            .zip(expected)
            .any(|(color, expected)| (color.red, color.green, color.blue) != expected)
    {
        return Err(Error::new(ErrorKind::InvalidData, "QueryColors changed RGB values").into());
    }
    Ok(())
}

fn wait_for_map<C: Connection>(
    connection: &C,
    window: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if let Event::MapNotify(event) = connection.wait_for_event()?
            && event.window == window
        {
            return Ok(());
        }
    }
}

fn draw_palette<C: Connection>(
    connection: &C,
    window: u32,
    gc: u32,
    pixels: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let row_bytes = usize::from(WIDTH) * 4;
    // Core PutImage is bounded by the setup request limit. Row-aligned chunks
    // preserve one exact image while remaining valid without BIG-REQUESTS.
    for row in (0..HEIGHT).step_by(usize::from(PUT_IMAGE_ROWS)) {
        let rows = PUT_IMAGE_ROWS.min(HEIGHT - row);
        let start = usize::from(row) * row_bytes;
        let end = start + usize::from(rows) * row_bytes;
        connection.put_image(
            ImageFormat::Z_PIXMAP,
            window,
            gc,
            WIDTH,
            rows,
            0,
            row as i16,
            0,
            24,
            &pixels[start..end],
        )?;
    }
    connection.flush()?;
    Ok(())
}

fn palette_bytes(order: ImageOrder) -> Vec<u8> {
    let mut row = Vec::with_capacity(usize::from(WIDTH) * 4);
    for (width, pixel) in BARS {
        let bytes = if order == ImageOrder::LSB_FIRST {
            pixel.to_le_bytes()
        } else {
            pixel.to_be_bytes()
        };
        for _ in 0..width {
            row.extend_from_slice(&bytes);
        }
    }
    debug_assert_eq!(row.len(), usize::from(WIDTH) * 4);
    row.repeat(usize::from(HEIGHT))
}
