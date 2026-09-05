const X_CREATE_WINDOW: u8 = 1;
const X_CHANGE_WINDOW_ATTRIBUTES: u8 = 2;
const X_GET_WINDOW_ATTRIBUTES: u8 = 3;
const X_DESTROY_WINDOW: u8 = 4;
const X_REPARENT_WINDOW: u8 = 7;
const X_MAP_WINDOW: u8 = 8;
const X_MAP_SUBWINDOWS: u8 = 9;
const X_UNMAP_WINDOW: u8 = 10;
const X_CONFIGURE_WINDOW: u8 = 12;
const X_GET_GEOMETRY: u8 = 14;
const X_QUERY_TREE: u8 = 15;
const X_INTERN_ATOM: u8 = 16;
const X_GET_ATOM_NAME: u8 = 17;
const X_CHANGE_PROPERTY: u8 = 18;
const X_DELETE_PROPERTY: u8 = 19;
const X_GET_PROPERTY: u8 = 20;
const X_QUERY_POINTER: u8 = 38;
const X_LIST_PROPERTIES: u8 = 21;
const X_SET_SELECTION_OWNER: u8 = 22;
const X_GET_SELECTION_OWNER: u8 = 23;
const X_CONVERT_SELECTION: u8 = 24;
const X_SEND_EVENT: u8 = 25;
const X_GRAB_POINTER: u8 = 26;
const X_UNGRAB_POINTER: u8 = 27;
const X_GRAB_BUTTON: u8 = 28;
const X_UNGRAB_BUTTON: u8 = 29;
const X_GRAB_KEYBOARD: u8 = 31;
const X_UNGRAB_KEYBOARD: u8 = 32;
const X_GRAB_KEY: u8 = 33;
const X_UNGRAB_KEY: u8 = 34;
const X_ALLOW_EVENTS: u8 = 35;
const X_GRAB_SERVER: u8 = 36;
const X_UNGRAB_SERVER: u8 = 37;
const X_TRANSLATE_COORDINATES: u8 = 40;
const X_SET_INPUT_FOCUS: u8 = 42;
const X_GET_INPUT_FOCUS: u8 = 43;
const X_OPEN_FONT: u8 = 45;
const X_CLOSE_FONT: u8 = 46;
const X_QUERY_FONT: u8 = 47;
const X_LIST_FONTS: u8 = 49;
const X_LIST_FONTS_WITH_INFO: u8 = 50;
const X_CREATE_PIXMAP: u8 = 53;
const X_FREE_PIXMAP: u8 = 54;
const X_CREATE_GC: u8 = 55;
const X_CHANGE_GC: u8 = 56;
const X_SET_CLIP_RECTANGLES: u8 = 59;
const X_FREE_GC: u8 = 60;
const X_CLEAR_AREA: u8 = 61;
const X_COPY_AREA: u8 = 62;
const X_POLY_LINE: u8 = 65;
const X_POLY_SEGMENT: u8 = 66;
const X_POLY_RECTANGLE: u8 = 67;
const X_FILL_POLY: u8 = 69;
const X_POLY_FILL_RECTANGLE: u8 = 70;
const X_POLY_FILL_ARC: u8 = 71;
const X_PUT_IMAGE: u8 = 72;
const X_GET_IMAGE: u8 = 73;
const X_POLY_TEXT8: u8 = 74;
const X_IMAGE_TEXT8: u8 = 76;
const X_CREATE_COLORMAP: u8 = 78;
const X_FREE_COLORMAP: u8 = 79;
const X_ALLOC_COLOR: u8 = 84;
const X_ALLOC_NAMED_COLOR: u8 = 85;
const X_QUERY_COLORS: u8 = 91;
const X_CREATE_CURSOR: u8 = 93;
const X_CREATE_GLYPH_CURSOR: u8 = 94;
const X_FREE_CURSOR: u8 = 95;
const X_RECOLOR_CURSOR: u8 = 96;
const X_QUERY_EXTENSION: u8 = 98;
const X_LIST_EXTENSIONS: u8 = 99;
const X_GET_KEYBOARD_MAPPING: u8 = 101;
const X_GET_KEYBOARD_CONTROL: u8 = 103;
const X_BELL: u8 = 104;
const X_GET_POINTER_MAPPING: u8 = 117;
const X_QUERY_BEST_SIZE: u8 = 97;
const X_GET_MODIFIER_MAPPING: u8 = 119;

pub const X_SOPHIA_PRESENT_EXTENSION_NAME: &str = "SOPHIA-PRESENT";
pub const X_SOPHIA_PRESENT_MAJOR_OPCODE: u8 = 130;
pub const X_SOPHIA_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_EXTENSION_NAME: &str = "MIT-SHM";
pub const X_MIT_SHM_MAJOR_OPCODE: u8 = 131;
pub const X_MIT_SHM_FIRST_EVENT: u8 = 108;
pub const X_MIT_SHM_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_MIT_SHM_ATTACH_MINOR_OPCODE: u8 = 1;
pub const X_MIT_SHM_DETACH_MINOR_OPCODE: u8 = 2;
pub const X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE: u8 = 3;
pub const X_MIT_SHM_GET_IMAGE_MINOR_OPCODE: u8 = 4;
pub const X_MIT_SHM_CREATE_PIXMAP_MINOR_OPCODE: u8 = 5;
/// MIT-SHM 1.2: the client passes a descriptor instead of a SysV id.
pub const X_MIT_SHM_ATTACH_FD_MINOR_OPCODE: u8 = 6;
/// MIT-SHM 1.2: the server allocates and hands the descriptor back.
pub const X_MIT_SHM_CREATE_SEGMENT_MINOR_OPCODE: u8 = 7;
pub const X_RANDR_EXTENSION_NAME: &str = "RANDR";
pub const X_RANDR_MAJOR_OPCODE: u8 = 132;
pub const X_RANDR_FIRST_EVENT: u8 = 64;
pub const X_RANDR_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_RANDR_SELECT_INPUT_MINOR_OPCODE: u8 = 4;
pub const X_RANDR_GET_SCREEN_SIZE_RANGE_MINOR_OPCODE: u8 = 6;
pub const X_RANDR_GET_SCREEN_RESOURCES_MINOR_OPCODE: u8 = 8;
pub const X_RANDR_GET_OUTPUT_INFO_MINOR_OPCODE: u8 = 9;
pub const X_RANDR_GET_OUTPUT_PROPERTY_MINOR_OPCODE: u8 = 15;
pub const X_RANDR_GET_CRTC_INFO_MINOR_OPCODE: u8 = 20;
pub const X_RANDR_GET_CRTC_GAMMA_SIZE_MINOR_OPCODE: u8 = 22;
pub const X_RANDR_GET_CRTC_GAMMA_MINOR_OPCODE: u8 = 23;
pub const X_RANDR_GET_SCREEN_RESOURCES_CURRENT_MINOR_OPCODE: u8 = 25;
pub const X_RANDR_GET_CRTC_TRANSFORM_MINOR_OPCODE: u8 = 27;
pub const X_RANDR_GET_PANNING_MINOR_OPCODE: u8 = 28;
pub const X_RANDR_GET_OUTPUT_PRIMARY_MINOR_OPCODE: u8 = 31;
pub const X_RANDR_GET_PROVIDERS_MINOR_OPCODE: u8 = 32;
pub const X_RANDR_GET_MONITORS_MINOR_OPCODE: u8 = 42;
pub const X_KEYBOARD_EXTENSION_NAME: &str = "XKEYBOARD";
pub const X_KEYBOARD_MAJOR_OPCODE: u8 = 133;
pub const X_KEYBOARD_FIRST_EVENT: u8 = 89;
pub const X_KEYBOARD_USE_EXTENSION_MINOR_OPCODE: u8 = 0;
pub const X_KEYBOARD_SELECT_EVENTS_MINOR_OPCODE: u8 = 1;
pub const X_KEYBOARD_GET_STATE_MINOR_OPCODE: u8 = 4;
pub const X_KEYBOARD_GET_CONTROLS_MINOR_OPCODE: u8 = 6;
pub const X_KEYBOARD_GET_MAP_MINOR_OPCODE: u8 = 8;
pub const X_KEYBOARD_GET_COMPAT_MAP_MINOR_OPCODE: u8 = 10;
pub const X_KEYBOARD_GET_INDICATOR_MAP_MINOR_OPCODE: u8 = 13;
pub const X_KEYBOARD_GET_NAMES_MINOR_OPCODE: u8 = 17;
pub const X_KEYBOARD_PER_CLIENT_FLAGS_MINOR_OPCODE: u8 = 21;
pub const X_KEYBOARD_GET_DEVICE_INFO_MINOR_OPCODE: u8 = 24;
pub const X_BIG_REQUESTS_EXTENSION_NAME: &str = "BIG-REQUESTS";
pub const X_BIG_REQUESTS_MAJOR_OPCODE: u8 = 134;
pub const X_BIG_REQUESTS_ENABLE_MINOR_OPCODE: u8 = 0;
pub const X_INPUT_EXTENSION_NAME: &str = "XInputExtension";
pub const X_INPUT_MAJOR_OPCODE: u8 = 135;
pub const X_INPUT_FIRST_EVENT: u8 = 90;
pub const X_INPUT_FIRST_ERROR: u8 = 160;
pub const X_INPUT_GET_EXTENSION_VERSION_MINOR_OPCODE: u8 = 1;
pub const X_INPUT_LIST_INPUT_DEVICES_MINOR_OPCODE: u8 = 2;
pub const X_INPUT_DEVICE_BELL_MINOR_OPCODE: u8 = 32;
/// XI1 `DeviceUse`: this device is the core pointer.
pub const X_INPUT_LEGACY_USE_POINTER: u8 = 0;
/// XI1 `DeviceUse`: this device is the core keyboard.
pub const X_INPUT_LEGACY_USE_KEYBOARD: u8 = 1;
/// XI1 `InputClass` discriminants. XI1 numbers its classes independently of XI2.
pub const X_INPUT_LEGACY_CLASS_KEY: u8 = 0;
pub const X_INPUT_LEGACY_CLASS_BUTTON: u8 = 1;
pub const X_INPUT_QUERY_POINTER_MINOR_OPCODE: u8 = 40;
pub const X_INPUT_CHANGE_CURSOR_MINOR_OPCODE: u8 = 42;
const X_INPUT_QUERY_POINTER_REQ_LEN: usize = 12;
const X_INPUT_CHANGE_CURSOR_REQ_LEN: usize = 16;
const X_INPUT_UNGRAB_DEVICE_REQ_LEN: usize = 12;
pub const X_INPUT_SELECT_EVENTS_MINOR_OPCODE: u8 = 46;
pub const X_INPUT_GET_CLIENT_POINTER_MINOR_OPCODE: u8 = 45;
pub const X_INPUT_QUERY_VERSION_MINOR_OPCODE: u8 = 47;
pub const X_INPUT_QUERY_DEVICE_MINOR_OPCODE: u8 = 48;
pub const X_INPUT_GET_FOCUS_MINOR_OPCODE: u8 = 50;
pub const X_INPUT_GRAB_DEVICE_MINOR_OPCODE: u8 = 51;
pub const X_INPUT_UNGRAB_DEVICE_MINOR_OPCODE: u8 = 52;
pub const X_INPUT_GET_PROPERTY_MINOR_OPCODE: u8 = 59;
pub const X_GENERIC_EVENT_EXTENSION_NAME: &str = "Generic Event Extension";
pub const X_GENERIC_EVENT_MAJOR_OPCODE: u8 = 136;
pub const X_GENERIC_EVENT_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_DRI3_EXTENSION_NAME: &str = "DRI3";
pub const X_DRI3_MAJOR_OPCODE: u8 = 137;
pub const X_DRI3_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_DRI3_OPEN_MINOR_OPCODE: u8 = 1;
pub const X_DRI3_PIXMAP_FROM_BUFFER_MINOR_OPCODE: u8 = 2;
pub const X_DRI3_BUFFER_FROM_PIXMAP_MINOR_OPCODE: u8 = 3;
pub const X_DRI3_FENCE_FROM_FD_MINOR_OPCODE: u8 = 4;
pub const X_DRI3_GET_SUPPORTED_MODIFIERS_MINOR_OPCODE: u8 = 6;
pub const X_DRI3_PIXMAP_FROM_BUFFERS_MINOR_OPCODE: u8 = 7;
pub const X_DRI3_BUFFERS_FROM_PIXMAP_MINOR_OPCODE: u8 = 8;
pub const X_PRESENT_EXTENSION_NAME: &str = "Present";
pub const X_PRESENT_MAJOR_OPCODE: u8 = 138;
pub const X_PRESENT_FIRST_EVENT: u8 = 0;
pub const X_PRESENT_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_PRESENT_PIXMAP_MINOR_OPCODE: u8 = 1;
pub const X_PRESENT_NOTIFY_MSC_MINOR_OPCODE: u8 = 2;
pub const X_PRESENT_SELECT_INPUT_MINOR_OPCODE: u8 = 3;
pub const X_PRESENT_QUERY_CAPABILITIES_MINOR_OPCODE: u8 = 4;
pub const X_XFIXES_EXTENSION_NAME: &str = "XFIXES";
pub const X_XFIXES_MAJOR_OPCODE: u8 = 139;
pub const X_XFIXES_FIRST_EVENT: u8 = 66;
pub const X_XFIXES_QUERY_VERSION_MINOR_OPCODE: u8 = 0;
pub const X_XFIXES_SELECT_SELECTION_INPUT_MINOR_OPCODE: u8 = 2;
pub const X_XFIXES_CREATE_REGION_MINOR_OPCODE: u8 = 5;
pub const X_XFIXES_DESTROY_REGION_MINOR_OPCODE: u8 = 10;
pub const X_XFIXES_SET_REGION_MINOR_OPCODE: u8 = 11;
pub const X_GLX_EXTENSION_NAME: &str = "GLX";
pub const X_GLX_MAJOR_OPCODE: u8 = 140;
pub const X_GLX_FIRST_EVENT: u8 = 72;

// The GLX request minors, all of them, in protocol order.
//
// The ones Sophia does not implement are named too. A table that lists only what
// is implemented cannot say what was refused, and it reads as though the rest of
// the protocol does not exist -- which for an extension advertising GLX 1.4 is a
// claim about the wrong thing. Each unimplemented minor carries why, so the
// distinction between "not offered" and "not yet written" survives in the table
// rather than in someone's memory.

/// Indirect GL rendering: the client streams GL commands to the server for
/// execution. Sophia runs no GL of its own -- every context here belongs to the
/// client's own driver -- so this is not a gap to be filled but a mode Sophia
/// does not offer.
pub const X_GLX_RENDER_MINOR_OPCODE: u8 = 1;
/// Indirect GL rendering for command buffers over the single-request limit. Not
/// offered, for the same reason as `Render`.
pub const X_GLX_RENDER_LARGE_MINOR_OPCODE: u8 = 2;
pub const X_GLX_CREATE_CONTEXT_MINOR_OPCODE: u8 = 3;
pub const X_GLX_DESTROY_CONTEXT_MINOR_OPCODE: u8 = 4;
pub const X_GLX_MAKE_CURRENT_MINOR_OPCODE: u8 = 5;
pub const X_GLX_IS_DIRECT_MINOR_OPCODE: u8 = 6;
pub const X_GLX_QUERY_VERSION_MINOR_OPCODE: u8 = 7;
/// Blocks until the indirect GL stream drains. Meaningless without a server-side
/// GL stream to drain.
pub const X_GLX_WAIT_GL_MINOR_OPCODE: u8 = 8;
/// Blocks until prior X requests complete, for ordering against indirect GL.
/// Same reason.
pub const X_GLX_WAIT_X_MINOR_OPCODE: u8 = 9;
/// Copies GL state between two server-side contexts. Sophia holds no GL state to
/// copy.
pub const X_GLX_COPY_CONTEXT_MINOR_OPCODE: u8 = 10;
/// The indirect present path. Direct clients reach the screen through DRI3 and
/// Present instead, which is the path Sophia implements.
pub const X_GLX_SWAP_BUFFERS_MINOR_OPCODE: u8 = 11;
/// Builds GL display lists from an X font. Indirect, and deprecated besides.
pub const X_GLX_USE_X_FONT_MINOR_OPCODE: u8 = 12;
/// GLX 1.2 GLX pixmaps. `GLX_DRAWABLE_TYPE` deliberately excludes
/// `GLX_PIXMAP_BIT`, so Sophia does not claim these drawables.
pub const X_GLX_CREATE_GLX_PIXMAP_MINOR_OPCODE: u8 = 13;
pub const X_GLX_GET_VISUAL_CONFIGS_MINOR_OPCODE: u8 = 14;
/// The destructor for GLX 1.2 GLX pixmaps, which are not offered.
pub const X_GLX_DESTROY_GLX_PIXMAP_MINOR_OPCODE: u8 = 15;
/// The vendor escape hatch, and the transport for extensions Sophia does not
/// advertise. Refusing it is what the advertisement already implies.
pub const X_GLX_VENDOR_PRIVATE_MINOR_OPCODE: u8 = 16;
/// `VendorPrivate` for escapes that answer. Not advertised, so not offered.
pub const X_GLX_VENDOR_PRIVATE_WITH_REPLY_MINOR_OPCODE: u8 = 17;
pub const X_GLX_QUERY_EXTENSIONS_STRING_MINOR_OPCODE: u8 = 18;
pub const X_GLX_QUERY_SERVER_STRING_MINOR_OPCODE: u8 = 19;
pub const X_GLX_CLIENT_INFO_MINOR_OPCODE: u8 = 20;
pub const X_GLX_GET_FB_CONFIGS_MINOR_OPCODE: u8 = 21;
/// GLX 1.3 GLX pixmaps, the FBConfig-based successor to minor 13. Excluded from
/// `GLX_DRAWABLE_TYPE` for the same reason.
pub const X_GLX_CREATE_PIXMAP_MINOR_OPCODE: u8 = 22;
/// The destructor for GLX 1.3 GLX pixmaps, which are not offered.
pub const X_GLX_DESTROY_PIXMAP_MINOR_OPCODE: u8 = 23;
pub const X_GLX_CREATE_NEW_CONTEXT_MINOR_OPCODE: u8 = 24;
pub const X_GLX_QUERY_CONTEXT_MINOR_OPCODE: u8 = 25;
pub const X_GLX_MAKE_CONTEXT_CURRENT_MINOR_OPCODE: u8 = 26;
pub const X_GLX_CREATE_PBUFFER_MINOR_OPCODE: u8 = 27;
pub const X_GLX_DESTROY_PBUFFER_MINOR_OPCODE: u8 = 28;
pub const X_GLX_GET_DRAWABLE_ATTRIBUTES_MINOR_OPCODE: u8 = 29;
pub const X_GLX_CHANGE_DRAWABLE_ATTRIBUTES_MINOR_OPCODE: u8 = 30;
pub const X_GLX_CREATE_WINDOW_MINOR_OPCODE: u8 = 31;
pub const X_GLX_DELETE_WINDOW_MINOR_OPCODE: u8 = 32;
pub const X_GLX_SET_CLIENT_INFO_ARB_MINOR_OPCODE: u8 = 33;
pub const X_GLX_CREATE_CONTEXT_ATTRIBS_ARB_MINOR_OPCODE: u8 = 34;
pub const X_GLX_SET_CLIENT_INFO_2_ARB_MINOR_OPCODE: u8 = 35;
/// The highest GLX minor the protocol defines. A minor above this is not a
/// request Sophia declined to implement; it is not a GLX request at all.
pub const X_GLX_LAST_MINOR_OPCODE: u8 = X_GLX_SET_CLIENT_INFO_2_ARB_MINOR_OPCODE;

// The GLX attribute tokens Sophia names, by the value that travels on the wire.
//
// These were thirty bare hex literals in the `GetFBConfigs` reply builder, where
// the only way to know that `0x186a1` was `GLX_SAMPLES` was to look it up. A
// reply built from unnamed numbers cannot be reviewed against the specification
// it claims to satisfy.

/// `GLX_BUFFER_SIZE`: total colour bits per pixel.
pub const X_GLX_BUFFER_SIZE_ATTRIBUTE: u32 = 0x2;
/// `GLX_LEVEL`: overlay/underlay plane, zero for the main plane.
pub const X_GLX_LEVEL_ATTRIBUTE: u32 = 0x3;
pub const X_GLX_DOUBLEBUFFER_ATTRIBUTE: u32 = 0x5;
pub const X_GLX_STEREO_ATTRIBUTE: u32 = 0x6;
pub const X_GLX_AUX_BUFFERS_ATTRIBUTE: u32 = 0x7;
pub const X_GLX_RED_SIZE_ATTRIBUTE: u32 = 0x8;
pub const X_GLX_GREEN_SIZE_ATTRIBUTE: u32 = 0x9;
pub const X_GLX_BLUE_SIZE_ATTRIBUTE: u32 = 0xA;
pub const X_GLX_ALPHA_SIZE_ATTRIBUTE: u32 = 0xB;
pub const X_GLX_DEPTH_SIZE_ATTRIBUTE: u32 = 0xC;
pub const X_GLX_STENCIL_SIZE_ATTRIBUTE: u32 = 0xD;
pub const X_GLX_ACCUM_RED_SIZE_ATTRIBUTE: u32 = 0xE;
pub const X_GLX_ACCUM_GREEN_SIZE_ATTRIBUTE: u32 = 0xF;
pub const X_GLX_ACCUM_BLUE_SIZE_ATTRIBUTE: u32 = 0x10;
pub const X_GLX_ACCUM_ALPHA_SIZE_ATTRIBUTE: u32 = 0x11;
pub const X_GLX_TRANSPARENT_TYPE_ATTRIBUTE: u32 = 0x20;
pub const X_GLX_X_VISUAL_TYPE_ATTRIBUTE: u32 = 0x22;
/// `GLX_CONFIG_CAVEAT`: whether choosing this configuration costs something.
pub const X_GLX_CONFIG_CAVEAT_ATTRIBUTE: u32 = 0x23;
pub const X_GLX_VISUAL_ID_ATTRIBUTE: u32 = 0x800B;
/// `GLX_DRAWABLE_TYPE`: which drawable kinds the configuration supports.
pub const X_GLX_DRAWABLE_TYPE_ATTRIBUTE: u32 = 0x8010;
/// `GLX_RENDER_TYPE`: which colour models the configuration renders.
pub const X_GLX_RENDER_TYPE_ATTRIBUTE: u32 = 0x8011;
/// `GLX_X_RENDERABLE`: whether X can draw to drawables of this configuration.
pub const X_GLX_X_RENDERABLE_ATTRIBUTE: u32 = 0x8012;
pub const X_GLX_FBCONFIG_ID_ATTRIBUTE: u32 = 0x8013;
pub const X_GLX_MAX_PBUFFER_WIDTH_ATTRIBUTE: u32 = 0x8016;
pub const X_GLX_MAX_PBUFFER_HEIGHT_ATTRIBUTE: u32 = 0x8017;
pub const X_GLX_MAX_PBUFFER_PIXELS_ATTRIBUTE: u32 = 0x8018;
/// `GLX_LARGEST_PBUFFER`: clamp to the maximum rather than refusing.
pub const X_GLX_LARGEST_PBUFFER_ATTRIBUTE: u32 = 0x801C;
/// `GLX_PBUFFER_WIDTH` and `GLX_PBUFFER_HEIGHT`. Height is the lower number.
pub const X_GLX_PBUFFER_HEIGHT_ATTRIBUTE: u32 = 0x8040;
pub const X_GLX_PBUFFER_WIDTH_ATTRIBUTE: u32 = 0x8041;
/// `GLX_FRAMEBUFFER_SRGB_CAPABLE_EXT`, from `GLX_EXT_framebuffer_sRGB`.
pub const X_GLX_FRAMEBUFFER_SRGB_CAPABLE_ATTRIBUTE: u32 = 0x20B2;
/// `GLX_SAMPLE_BUFFERS`, added by GLX 1.4. Answered as zero: Sophia offers no
/// multisample configuration, and says so rather than omitting the attribute.
pub const X_GLX_SAMPLE_BUFFERS_ATTRIBUTE: u32 = 0x186A0;
/// `GLX_SAMPLES`, added by GLX 1.4. Zero, for the same reason.
pub const X_GLX_SAMPLES_ATTRIBUTE: u32 = 0x186A1;

// The GLX attribute values Sophia answers with.

/// `GLX_NONE`: the answer for both `GLX_TRANSPARENT_TYPE` and
/// `GLX_CONFIG_CAVEAT`.
pub const X_GLX_NONE_VALUE: u32 = 0x8000;
/// `GLX_TRUE_COLOR`, the only X visual type Sophia offers.
pub const X_GLX_TRUE_COLOR_VALUE: u32 = 0x8002;
/// `GLX_RGBA_BIT`: the only render type Sophia offers. Colour-index rendering
/// went with indirect GL.
pub const X_GLX_RGBA_BIT_VALUE: u32 = 0x1;
pub const X_SYNC_EXTENSION_NAME: &str = "SYNC";
pub const X_SYNC_MAJOR_OPCODE: u8 = 141;
pub const X_SYNC_FIRST_EVENT: u8 = 68;
pub const X_SYNC_INITIALIZE_MINOR_OPCODE: u8 = 0;
pub const X_SYNC_LIST_SYSTEM_COUNTERS_MINOR_OPCODE: u8 = 1;
pub const X_SYNC_CREATE_COUNTER_MINOR_OPCODE: u8 = 2;
pub const X_SYNC_SET_COUNTER_MINOR_OPCODE: u8 = 3;
pub const X_SYNC_CHANGE_COUNTER_MINOR_OPCODE: u8 = 4;
pub const X_SYNC_QUERY_COUNTER_MINOR_OPCODE: u8 = 5;
pub const X_SYNC_DESTROY_COUNTER_MINOR_OPCODE: u8 = 6;
pub const X_SYNC_DESTROY_FENCE_MINOR_OPCODE: u8 = 17;

const X_CREATE_WINDOW_REQ_LEN: usize = 32;
const X_CHANGE_WINDOW_ATTRIBUTES_REQ_LEN: usize = 12;
const X_GET_WINDOW_ATTRIBUTES_REQ_LEN: usize = 8;
const X_DESTROY_WINDOW_REQ_LEN: usize = 8;
const X_REPARENT_WINDOW_REQ_LEN: usize = 16;
const X_MAP_WINDOW_REQ_LEN: usize = 8;
const X_MAP_SUBWINDOWS_REQ_LEN: usize = 8;
const X_UNMAP_WINDOW_REQ_LEN: usize = 8;
const X_CONFIGURE_WINDOW_REQ_LEN: usize = 12;
const X_GET_GEOMETRY_REQ_LEN: usize = 8;
const X_QUERY_TREE_REQ_LEN: usize = 8;
const X_INTERN_ATOM_REQ_LEN: usize = 8;
const X_GET_ATOM_NAME_REQ_LEN: usize = 8;
const X_CHANGE_PROPERTY_REQ_LEN: usize = 24;
const X_DELETE_PROPERTY_REQ_LEN: usize = 12;
const X_GET_PROPERTY_REQ_LEN: usize = 24;
const X_QUERY_POINTER_REQ_LEN: usize = 8;
const X_LIST_PROPERTIES_REQ_LEN: usize = 8;
const X_SET_SELECTION_OWNER_REQ_LEN: usize = 16;
const X_GET_SELECTION_OWNER_REQ_LEN: usize = 8;
const X_CONVERT_SELECTION_REQ_LEN: usize = 24;
const X_SEND_EVENT_REQ_LEN: usize = 44;
const X_GRAB_BUTTON_REQ_LEN: usize = 24;
const X_UNGRAB_BUTTON_REQ_LEN: usize = 12;
const X_GRAB_POINTER_REQ_LEN: usize = 24;
const X_UNGRAB_POINTER_REQ_LEN: usize = 8;
const X_GRAB_KEYBOARD_REQ_LEN: usize = 16;
const X_UNGRAB_KEYBOARD_REQ_LEN: usize = 8;
const X_GRAB_KEY_REQ_LEN: usize = 16;
const X_UNGRAB_KEY_REQ_LEN: usize = 12;
const X_ALLOW_EVENTS_REQ_LEN: usize = 8;
const X_GRAB_SERVER_REQ_LEN: usize = 4;
const X_UNGRAB_SERVER_REQ_LEN: usize = 4;
const X_TRANSLATE_COORDINATES_REQ_LEN: usize = 16;
const X_SET_INPUT_FOCUS_REQ_LEN: usize = 12;
const X_GET_INPUT_FOCUS_REQ_LEN: usize = 4;
const X_GET_IMAGE_REQ_LEN: usize = 20;
const X_OPEN_FONT_REQ_LEN: usize = 12;
const X_CLOSE_FONT_REQ_LEN: usize = 8;
const X_QUERY_FONT_REQ_LEN: usize = 8;
const X_LIST_FONTS_REQ_LEN: usize = 8;
const X_LIST_FONTS_WITH_INFO_REQ_LEN: usize = 8;
const X_CREATE_PIXMAP_REQ_LEN: usize = 16;
const X_FREE_PIXMAP_REQ_LEN: usize = 8;
const X_CREATE_GC_REQ_LEN: usize = 16;
const X_CHANGE_GC_REQ_LEN: usize = 12;
const X_SET_CLIP_RECTANGLES_REQ_LEN: usize = 12;
const X_FREE_GC_REQ_LEN: usize = 8;
const X_CLEAR_AREA_REQ_LEN: usize = 16;
const X_COPY_AREA_REQ_LEN: usize = 28;
const X_POLY_LINE_REQ_LEN: usize = 12;
const X_POLY_SEGMENT_REQ_LEN: usize = 12;
const X_POLY_RECTANGLE_REQ_LEN: usize = 12;
const X_FILL_POLY_REQ_LEN: usize = 16;
const X_POLY_FILL_RECTANGLE_REQ_LEN: usize = 12;
const X_POLY_FILL_ARC_REQ_LEN: usize = 12;
const X_PUT_IMAGE_REQ_LEN: usize = 24;
const X_POLY_TEXT8_REQ_LEN: usize = 16;
const X_IMAGE_TEXT8_REQ_LEN: usize = 16;
const X_CREATE_COLORMAP_REQ_LEN: usize = 16;
const X_FREE_COLORMAP_REQ_LEN: usize = 8;
const X_ALLOC_COLOR_REQ_LEN: usize = 16;
const X_ALLOC_NAMED_COLOR_REQ_LEN: usize = 12;
const X_QUERY_COLORS_REQ_LEN: usize = 8;
const X_CREATE_CURSOR_REQ_LEN: usize = 32;
const X_CREATE_GLYPH_CURSOR_REQ_LEN: usize = 32;
const X_FREE_CURSOR_REQ_LEN: usize = 8;
const X_RECOLOR_CURSOR_REQ_LEN: usize = 20;
const X_QUERY_EXTENSION_REQ_LEN: usize = 8;
const X_LIST_EXTENSIONS_REQ_LEN: usize = 4;
const X_GET_KEYBOARD_MAPPING_REQ_LEN: usize = 8;
const X_GET_POINTER_MAPPING_REQ_LEN: usize = 4;
const X_QUERY_BEST_SIZE_REQ_LEN: usize = 12;
const X_GET_MODIFIER_MAPPING_REQ_LEN: usize = 4;
const X_SOPHIA_PRESENT_PIXMAP_REQ_LEN: usize = 32;
const X_MIT_SHM_QUERY_VERSION_REQ_LEN: usize = 4;
const X_MIT_SHM_ATTACH_REQ_LEN: usize = 16;
const X_MIT_SHM_DETACH_REQ_LEN: usize = 8;
const X_MIT_SHM_PUT_IMAGE_REQ_LEN: usize = 40;
const X_MIT_SHM_GET_IMAGE_REQ_LEN: usize = 32;
const X_MIT_SHM_CREATE_PIXMAP_REQ_LEN: usize = 28;
/// The descriptor arrives out of band, so it occupies no request bytes.
const X_MIT_SHM_ATTACH_FD_REQ_LEN: usize = 12;
const X_MIT_SHM_CREATE_SEGMENT_REQ_LEN: usize = 16;
const X_RANDR_QUERY_VERSION_REQ_LEN: usize = 12;
const X_RANDR_SELECT_INPUT_REQ_LEN: usize = 12;
const X_RANDR_GET_SCREEN_SIZE_RANGE_REQ_LEN: usize = 8;
const X_RANDR_GET_SCREEN_RESOURCES_REQ_LEN: usize = 8;
const X_RANDR_GET_OUTPUT_INFO_REQ_LEN: usize = 12;
const X_RANDR_GET_OUTPUT_PROPERTY_REQ_LEN: usize = 28;
const X_RANDR_GET_CRTC_INFO_REQ_LEN: usize = 12;
const X_RANDR_GET_CRTC_GAMMA_SIZE_REQ_LEN: usize = 8;
const X_RANDR_GET_CRTC_GAMMA_REQ_LEN: usize = 8;
const X_RANDR_GET_CRTC_TRANSFORM_REQ_LEN: usize = 8;
const X_RANDR_GET_PANNING_REQ_LEN: usize = 8;
const X_RANDR_GET_OUTPUT_PRIMARY_REQ_LEN: usize = 8;
const X_RANDR_GET_MONITORS_REQ_LEN: usize = 12;
const X_KEYBOARD_USE_EXTENSION_REQ_LEN: usize = 8;
const X_KEYBOARD_SELECT_EVENTS_REQ_LEN: usize = 16;
const X_KEYBOARD_GET_MAP_REQ_LEN: usize = 28;
const X_KEYBOARD_GET_CONTROLS_REQ_LEN: usize = 8;
const X_KEYBOARD_PER_CLIENT_FLAGS_REQ_LEN: usize = 28;
const X_BIG_REQUESTS_ENABLE_REQ_LEN: usize = 4;
const X_INPUT_LIST_INPUT_DEVICES_REQ_LEN: usize = 4;
const X_INPUT_QUERY_VERSION_REQ_LEN: usize = 8;
const X_INPUT_GET_CLIENT_POINTER_REQ_LEN: usize = 8;
const X_INPUT_QUERY_DEVICE_REQ_LEN: usize = 8;
const X_INPUT_SELECT_EVENTS_REQ_LEN: usize = 12;
const X_INPUT_GET_FOCUS_REQ_LEN: usize = 8;
const X_INPUT_GRAB_DEVICE_REQ_LEN: usize = 24;
const X_INPUT_GET_PROPERTY_REQ_LEN: usize = 24;
const X_GENERIC_EVENT_QUERY_VERSION_REQ_LEN: usize = 8;

pub const X_PUT_IMAGE_MAX_DATA_BYTES: usize = 256 * 1024;
pub const X_QUERY_COLORS_MAX_PIXELS: usize = 256;
pub const X_POLY_TEXT8_MAX_BYTES: usize = 64 * 1024;
pub const X_IMAGE_TEXT8_MAX_BYTES: usize = 64 * 1024;
pub const X_ALLOC_NAMED_COLOR_MAX_NAME_BYTES: usize = 256;
