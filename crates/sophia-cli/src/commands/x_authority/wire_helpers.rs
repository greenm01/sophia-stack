fn x11_setup_request(byte_order: XByteOrder) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(byte_order.marker());
    out.push(0);
    push_x11_u16(&mut out, byte_order, 11);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 0);
    out
}

fn x11_create_window_request(
    byte_order: XByteOrder,
    window: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Vec<u8> {
    let mut out = vec![1, 24];
    push_x11_u16(&mut out, byte_order, 8);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, 0x20);
    push_x11_i16(&mut out, byte_order, x);
    push_x11_i16(&mut out, byte_order, y);
    push_x11_u16(&mut out, byte_order, width);
    push_x11_u16(&mut out, byte_order, height);
    push_x11_u16(&mut out, byte_order, 0);
    push_x11_u16(&mut out, byte_order, 1);
    push_x11_u32(&mut out, byte_order, 0);
    push_x11_u32(&mut out, byte_order, 0);
    out
}

fn x11_resource_request(byte_order: XByteOrder, opcode: u8, id: u32) -> Vec<u8> {
    let mut out = vec![opcode, 0];
    push_x11_u16(&mut out, byte_order, 2);
    push_x11_u32(&mut out, byte_order, id);
    out
}

fn x11_intern_atom_request(byte_order: XByteOrder, only_if_exists: bool, name: &str) -> Vec<u8> {
    let mut out = vec![16, u8::from(only_if_exists)];
    let len_units = (8 + padded_x11_len(name.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u16(&mut out, byte_order, name.len() as u16);
    push_x11_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_x11(&mut out);
    out
}

fn x11_query_extension_request(byte_order: XByteOrder, name: &str) -> Vec<u8> {
    let mut out = vec![98, 0];
    let len_units = (8 + padded_x11_len(name.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u16(&mut out, byte_order, name.len() as u16);
    push_x11_u16(&mut out, byte_order, 0);
    out.extend_from_slice(name.as_bytes());
    pad_x11(&mut out);
    out
}

fn x11_sophia_present_pixmap_request(
    byte_order: XByteOrder,
    window: u32,
    pixmap: u32,
    damage: (i16, i16, u16, u16),
    previous_committed_generation: u64,
    timeout_msec: u32,
) -> Vec<u8> {
    let mut out = vec![
        X_SOPHIA_PRESENT_MAJOR_OPCODE,
        X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE,
    ];
    push_x11_u16(&mut out, byte_order, 8);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, pixmap);
    push_x11_i16(&mut out, byte_order, damage.0);
    push_x11_i16(&mut out, byte_order, damage.1);
    push_x11_u16(&mut out, byte_order, damage.2);
    push_x11_u16(&mut out, byte_order, damage.3);
    push_x11_u64(&mut out, byte_order, previous_committed_generation);
    push_x11_u32(&mut out, byte_order, timeout_msec);
    out
}

fn x11_change_property_request(
    byte_order: XByteOrder,
    window: u32,
    property: u32,
    property_type: u32,
    bytes: &[u8],
) -> Vec<u8> {
    let mut out = vec![18, 0];
    let len_units = (24 + padded_x11_len(bytes.len())) / 4;
    push_x11_u16(&mut out, byte_order, len_units as u16);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, property);
    push_x11_u32(&mut out, byte_order, property_type);
    out.push(8);
    out.extend_from_slice(&[0, 0, 0]);
    push_x11_u32(&mut out, byte_order, bytes.len() as u32);
    out.extend_from_slice(bytes);
    pad_x11(&mut out);
    out
}

fn x11_get_property_request(
    byte_order: XByteOrder,
    window: u32,
    property: u32,
    property_type: u32,
    long_offset: u32,
    long_length: u32,
) -> Vec<u8> {
    let mut out = vec![20, 0];
    push_x11_u16(&mut out, byte_order, 6);
    push_x11_u32(&mut out, byte_order, window);
    push_x11_u32(&mut out, byte_order, property);
    push_x11_u32(&mut out, byte_order, property_type);
    push_x11_u32(&mut out, byte_order, long_offset);
    push_x11_u32(&mut out, byte_order, long_length);
    out
}

fn read_x11_setup_success(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut prefix = [0; 8];
    stream.read_exact(&mut prefix)?;
    if prefix[0] != 1 {
        return Err(format!("X11 setup failed with status {}", prefix[0]).into());
    }
    let body_len = usize::from(read_x11_u16(byte_order, &prefix[6..8])) * 4;
    let mut body = vec![0; body_len];
    stream.read_exact(&mut body)?;
    Ok(())
}

fn read_x11_record(stream: &mut UnixStream) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut record = [0; 32];
    stream.read_exact(&mut record)?;
    Ok(record)
}

fn read_x11_reply(
    stream: &mut UnixStream,
    byte_order: XByteOrder,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;

    let mut prefix = [0; 32];
    stream.read_exact(&mut prefix)?;
    let body_len = usize::try_from(read_x11_u32(byte_order, &prefix[4..8]))? * 4;
    let mut reply = prefix.to_vec();
    reply.resize(32 + body_len, 0);
    stream.read_exact(&mut reply[32..])?;
    Ok(reply)
}

fn push_x11_u16(out: &mut Vec<u8>, byte_order: XByteOrder, value: u16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_i16(out: &mut Vec<u8>, byte_order: XByteOrder, value: i16) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_u32(out: &mut Vec<u8>, byte_order: XByteOrder, value: u32) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn push_x11_u64(out: &mut Vec<u8>, byte_order: XByteOrder, value: u64) {
    match byte_order {
        XByteOrder::LittleEndian => out.extend_from_slice(&value.to_le_bytes()),
        XByteOrder::BigEndian => out.extend_from_slice(&value.to_be_bytes()),
    }
}

fn read_x11_u16(byte_order: XByteOrder, bytes: &[u8]) -> u16 {
    match byte_order {
        XByteOrder::LittleEndian => u16::from_le_bytes(bytes.try_into().expect("u16 bytes")),
        XByteOrder::BigEndian => u16::from_be_bytes(bytes.try_into().expect("u16 bytes")),
    }
}

fn read_x11_u32(byte_order: XByteOrder, bytes: &[u8]) -> u32 {
    match byte_order {
        XByteOrder::LittleEndian => u32::from_le_bytes(bytes.try_into().expect("u32 bytes")),
        XByteOrder::BigEndian => u32::from_be_bytes(bytes.try_into().expect("u32 bytes")),
    }
}

fn pad_x11(out: &mut Vec<u8>) {
    out.resize(padded_x11_len(out.len()), 0);
}

const fn padded_x11_len(len: usize) -> usize {
    (len + 3) & !3
}

fn send_request(
    stream: &mut UnixStream,
    request: XAuthorityRequestPacket,
) -> Result<sophia_x_authority::XAuthorityResponsePacket, Box<dyn std::error::Error>> {
    write_x_authority_request(stream, &request)?;
    Ok(read_x_authority_response(stream)?)
}

fn temp_xauthority_display(
    base: u32,
) -> Result<(String, std::path::PathBuf), Box<dyn std::error::Error>> {
    let display_number = base + (std::process::id() % 1000);
    let display = format!(":{display_number}");
    let socket_path = std::path::PathBuf::from(format!("/tmp/.X11-unix/X{display_number}"));
    std::fs::create_dir_all("/tmp/.X11-unix")?;
    Ok((display, socket_path))
}

fn run_compiled_xlib_probe(
    display: &str,
    name: &str,
    source: &str,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let source_path = std::env::temp_dir().join(format!(
        "sophia-xauthority-{name}-{}-{}.c",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let binary_path = source_path.with_extension("bin");
    std::fs::write(&source_path, source)?;
    let compile = std::process::Command::new("gcc")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .arg("-lX11")
        .output()?;
    if !compile.status.success() {
        let _ = std::fs::remove_file(&source_path);
        return Err(format!(
            "failed to compile {name} smoke: {}",
            String::from_utf8_lossy(&compile.stderr).trim()
        )
        .into());
    }
    let output = std::process::Command::new(&binary_path)
        .env("DISPLAY", display)
        .output()?;
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&binary_path);
    Ok(output)
}

fn run_xkbcommon_x11_probe(
    display: &str,
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let source_path = std::env::temp_dir().join(format!(
        "sophia-xkbcommon-x11-{}-{}.c",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let binary_path = source_path.with_extension("bin");
    std::fs::write(&source_path, XKBCOMMON_X11_PROBE_SOURCE)?;
    let compile = std::process::Command::new("gcc")
        .arg(&source_path)
        .arg("-o")
        .arg(&binary_path)
        .args(["-lxkbcommon-x11", "-lxkbcommon", "-lxcb", "-lxcb-xkb"])
        .output()?;
    if !compile.status.success() {
        let _ = std::fs::remove_file(&source_path);
        return Err(format!(
            "failed to compile xkbcommon-x11 probe: {}",
            String::from_utf8_lossy(&compile.stderr).trim()
        )
        .into());
    }
    let output = std::process::Command::new(&binary_path)
        .env("DISPLAY", display)
        .output()?;
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&binary_path);
    Ok(output)
}

fn xlib_smoke_title_bytes(stdout: &str) -> Option<usize> {
    xlib_smoke_field(stdout, "title_bytes")
}

fn xlib_smoke_field(stdout: &str, name: &str) -> Option<usize> {
    let prefix = format!("{name}=");
    stdout
        .split_whitespace()
        .find_map(|field| field.strip_prefix(&prefix))
        .and_then(|value| value.parse().ok())
}

pub(crate) fn wait_for_socket_path(
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    Err(format!(
        "timed out waiting for X authority socket {}",
        path.display()
    )
    .into())
}

const XKBCOMMON_X11_PROBE_SOURCE: &str = r#"
#include <stdio.h>
#include <xcb/xcb.h>
#include <xkbcommon/xkbcommon.h>
#include <xkbcommon/xkbcommon-x11.h>

int main(void) {
    xcb_connection_t *connection = xcb_connect(NULL, NULL);
    if (!connection || xcb_connection_has_error(connection)) return 2;
    if (!xkb_x11_setup_xkb_extension(
            connection, 1, 0, XKB_X11_SETUP_XKB_EXTENSION_NO_FLAGS,
            NULL, NULL, NULL, NULL)) return 3;
    int32_t device = xkb_x11_get_core_keyboard_device_id(connection);
    if (device < 0) return 4;
    struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    struct xkb_keymap *keymap = xkb_x11_keymap_new_from_device(
        context, connection, device, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!keymap) return 5;
    struct xkb_state *state = xkb_x11_state_new_from_device(keymap, connection, device);
    if (!state) return 6;
    xkb_keysym_t sym = xkb_state_key_get_one_sym(state, 46);
    char name[64] = {0};
    xkb_keysym_get_name(sym, name, sizeof(name));
    printf("device=%d keycode=46 keysym=%s raw=%u\n", device, name, sym);
    xkb_state_unref(state);
    xkb_keymap_unref(keymap);
    xkb_context_unref(context);
    xcb_disconnect(connection);
    return 0;
}
"#;

const XLIB_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 240, 160, 0, 0, 0);
    Atom net_wm_name = XInternAtom(display, "_NET_WM_NAME", False);
    Atom utf8 = XInternAtom(display, "UTF8_STRING", False);
    const char *title = "Sophia Xlib";
    XStoreName(display, window, title);
    XChangeProperty(display, window, net_wm_name, utf8, 8, PropModeReplace,
                    (const unsigned char *)title, (int)strlen(title));

    Atom actual_type = None;
    int actual_format = 0;
    unsigned long nitems = 0;
    unsigned long bytes_after = 0;
    unsigned char *value = NULL;
    int property_status = XGetWindowProperty(display, window, net_wm_name, 0, 64, False,
                                             AnyPropertyType, &actual_type, &actual_format,
                                             &nitems, &bytes_after, &value);
    if (property_status != Success) {
        fprintf(stderr, "get_property=%d\n", property_status);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 3;
    }

    int title_match = value != NULL && nitems == strlen(title) &&
        memcmp(value, title, strlen(title)) == 0;
    if (value) {
        XFree(value);
    }

    XMapWindow(display, window);
    XSync(display, False);
    printf("window=0x%lx title_bytes=%lu title_match=%d\n", window, nitems, title_match);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return title_match ? 0 : 4;
}
"#;

const XLIB_DRAWING_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <stdio.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 240, 160, 0, 0, 0);
    GC gc = XCreateGC(display, window, 0, NULL);
    XMapWindow(display, window);
    XFillRectangle(display, window, gc, 5, 6, 40, 30);
    XSync(display, False);
    printf("window=0x%lx draw_ops=1\n", window);
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
"#;

const XLIB_PUT_IMAGE_SMOKE_SOURCE: &str = r#"
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "open_display=0\n");
        return 2;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(display, root, 10, 20, 400, 200, 0, 0, 0);
    GC gc = XCreateGC(display, window, 0, NULL);
    XMapWindow(display, window);

    const int width = 8;
    const int height = 4;
    char *data = calloc((size_t)width * (size_t)height, 4);
    if (!data) {
        fprintf(stderr, "alloc=0\n");
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 3;
    }
    for (int i = 0; i < width * height * 4; ++i) {
        data[i] = (char)(i * 3);
    }

    XImage *image = XCreateImage(display, DefaultVisual(display, screen),
                                 DefaultDepth(display, screen), ZPixmap, 0,
                                 data, width, height, 32, 0);
    if (!image) {
        fprintf(stderr, "create_image=0\n");
        free(data);
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 4;
    }

    XPutImage(display, window, gc, image, 0, 0, 3, 5, width, height);
    XSync(display, False);

    unsigned long expected = XGetPixel(image, 0, 0);
    XImage *readback = XGetImage(display, window, 0, 0, 400, 200,
                                 AllPlanes, ZPixmap);
    if (!readback) {
        fprintf(stderr, "get_image=0\n");
        XDestroyImage(image);
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 5;
    }
    unsigned long actual = XGetPixel(readback, 3, 5);
    if (actual != expected) {
        fprintf(stderr, "pixel_match=0 expected=0x%lx actual=0x%lx\n",
                expected, actual);
        XDestroyImage(readback);
        XDestroyImage(image);
        XFreeGC(display, gc);
        XDestroyWindow(display, window);
        XCloseDisplay(display);
        return 6;
    }
    printf("window=0x%lx image_ops=2 readback_bytes=320000\n", window);

    XDestroyImage(readback);
    XDestroyImage(image);
    XFreeGC(display, gc);
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
"#;
