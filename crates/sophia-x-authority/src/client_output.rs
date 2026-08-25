use crate::{
    X_ATOM_NONE, XAuthorityRuntimeError, XByteOrder, XColorRgb16, XResourceId, XTimestamp,
    XWireParseError, padded_len,
};
use sophia_protocol::Rect;

include!("client_output/replies/core_early.rs");
include!("client_output/replies/core_late.rs");
include!("client_output/replies/glx_sync.rs");
include!("client_output/replies/randr.rs");
include!("client_output/replies/render_extensions.rs");
include!("client_output/replies/xi.rs");
include!("client_output/replies/xkb.rs");
include!("client_output/errors.rs");
include!("client_output/events.rs");
include!("client_output/helpers.rs");

pub const X_CLIENT_OUTPUT_RECORD_LEN: usize = 32;

const X_ERROR: u8 = 0;
const X_KEY_PRESS: u8 = 2;
const X_KEY_RELEASE: u8 = 3;
const X_BUTTON_PRESS: u8 = 4;
const X_BUTTON_RELEASE: u8 = 5;
const X_MOTION_NOTIFY: u8 = 6;
const X_FOCUS_IN: u8 = 9;
const X_FOCUS_OUT: u8 = 10;
const X_EXPOSE: u8 = 12;
const X_VISIBILITY_NOTIFY: u8 = 15;
const X_UNMAP_NOTIFY: u8 = 18;
const X_MAP_NOTIFY: u8 = 19;
const X_CONFIGURE_NOTIFY: u8 = 22;
const X_PROPERTY_NOTIFY: u8 = 28;
const X_SELECTION_NOTIFY: u8 = 31;

const PROPERTY_NEW_VALUE: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XErrorCode {
    BadRequest,
    BadValue,
    BadWindow,
    BadPixmap,
    BadDrawable,
    BadAtom,
    BadFont,
    BadMatch,
    BadAccess,
    BadAlloc,
    BadColor,
    BadGraphicsContext,
    BadIdChoice,
    BadName,
    BadLength,
    BadImplementation,
}

impl XErrorCode {
    pub const fn wire_code(self) -> u8 {
        match self {
            Self::BadRequest => 1,
            Self::BadValue => 2,
            Self::BadWindow => 3,
            Self::BadPixmap => 4,
            Self::BadDrawable => 9,
            Self::BadAtom => 5,
            Self::BadFont => 7,
            Self::BadMatch => 8,
            Self::BadAccess => 10,
            Self::BadAlloc => 11,
            Self::BadColor => 12,
            Self::BadGraphicsContext => 13,
            Self::BadIdChoice => 14,
            Self::BadName => 15,
            Self::BadLength => 16,
            Self::BadImplementation => 17,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XClientError {
    pub code: XErrorCode,
    pub sequence: u16,
    pub resource_id: u32,
    pub minor_code: u16,
    pub major_code: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XClientEvent {
    Key {
        sequence: u16,
        pressed: bool,
        keycode: u8,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        state: u16,
    },
    Focus {
        sequence: u16,
        focused: bool,
        detail: u8,
        event: XResourceId,
        mode: u8,
    },
    XkbStateNotify {
        sequence: u16,
        time: XTimestamp,
        modifiers: u8,
        changed: u16,
        keycode: u8,
        event_type: u8,
    },
    PointerMotion {
        sequence: u16,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
    },
    PointerButton {
        sequence: u16,
        pressed: bool,
        button: u8,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
    },
    PointerCrossing {
        sequence: u16,
        entered: bool,
        detail: u8,
        time: XTimestamp,
        root: XResourceId,
        event: XResourceId,
        root_x: i16,
        root_y: i16,
        event_x: i16,
        event_y: i16,
        state: u16,
        mode: u8,
        focus: bool,
    },
    Expose {
        sequence: u16,
        window: XResourceId,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        count: u16,
    },
    VisibilityNotify {
        sequence: u16,
        window: XResourceId,
        state: u8,
    },
    CreateNotify {
        sequence: u16,
        parent: XResourceId,
        window: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool,
    },
    MapNotify {
        sequence: u16,
        event: XResourceId,
        window: XResourceId,
        override_redirect: bool,
    },
    UnmapNotify {
        sequence: u16,
        event: XResourceId,
        window: XResourceId,
        from_configure: bool,
    },
    ConfigureNotify {
        sequence: u16,
        synthetic: bool,
        event: XResourceId,
        window: XResourceId,
        above_sibling: Option<XResourceId>,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        border_width: u16,
        override_redirect: bool,
    },
    PropertyNotify {
        sequence: u16,
        window: XResourceId,
        atom: u32,
        time: XTimestamp,
        new_value: bool,
    },
    SelectionClear {
        sequence: u16,
        time: XTimestamp,
        owner: XResourceId,
        selection: u32,
    },
    SelectionRequest {
        sequence: u16,
        time: XTimestamp,
        owner: XResourceId,
        requestor: XResourceId,
        selection: u32,
        target: u32,
        property: u32,
    },
    SelectionNotify {
        sequence: u16,
        synthetic: bool,
        time: XTimestamp,
        requestor: XResourceId,
        selection: u32,
        target: u32,
        property: u32,
    },
    ClientMessage {
        sequence: u16,
        bytes: [u8; X_CLIENT_OUTPUT_RECORD_LEN],
    },
    ShmCompletion {
        sequence: u16,
        drawable: XResourceId,
        segment: XResourceId,
        offset: u32,
    },
    PresentConfigureNotify {
        sequence: u16,
        event_id: XResourceId,
        window: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        pixmap_width: u16,
        pixmap_height: u16,
        pixmap_flags: u32,
    },
    PresentCompleteNotify {
        sequence: u16,
        event_id: XResourceId,
        window: XResourceId,
        serial: u32,
        ust: u64,
        msc: u64,
        /// 0 = a presented pixmap completed; 1 = an MSC notification.
        kind: u8,
        mode: u8,
    },
    PresentIdleNotify {
        sequence: u16,
        event_id: XResourceId,
        window: XResourceId,
        serial: u32,
        pixmap: XResourceId,
        idle_fence: Option<XResourceId>,
    },
    RandrScreenChange {
        sequence: u16,
        timestamp: u32,
        config_timestamp: u32,
        root: XResourceId,
        request_window: XResourceId,
        width: u16,
        height: u16,
        mm_width: u16,
        mm_height: u16,
    },
    RandrCrtcChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
        crtc: u32,
        mode: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    },
    RandrOutputChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
        output: u32,
        crtc: u32,
        mode: u32,
    },
    RandrResourceChange {
        sequence: u16,
        timestamp: u32,
        window: XResourceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XClientReply {
    GrabStatus {
        sequence: u16,
        status: u8,
    },
    InternAtom {
        sequence: u16,
        atom: u32,
    },
    GetAtomName {
        sequence: u16,
        name: String,
    },
    GetGeometry {
        sequence: u16,
        depth: u8,
        root: XResourceId,
        geometry: Rect,
        border_width: u16,
    },
    GetImage {
        sequence: u16,
        depth: u8,
        visual: u32,
        data: Vec<u8>,
    },
    QueryTree {
        sequence: u16,
        root: XResourceId,
        parent: XResourceId,
        children: Vec<XResourceId>,
    },
    GetWindowAttributes {
        sequence: u16,
        visual: u32,
        colormap: XResourceId,
        map_state: u8,
        override_redirect: bool,
    },
    QueryExtension {
        sequence: u16,
        present: bool,
        major_opcode: u8,
        first_event: u8,
        first_error: u8,
    },
    ListExtensions {
        sequence: u16,
    },
    ListFonts {
        sequence: u16,
        names: Vec<String>,
    },
    ListFontsWithInfo {
        sequence: u16,
        names: Vec<String>,
    },
    QueryBestSize {
        sequence: u16,
        width: u16,
        height: u16,
    },
    ShmQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
        shared_pixmaps: bool,
        pixmap_format: u8,
    },
    ShmGetImage {
        sequence: u16,
        depth: u8,
        visual: u32,
        size: u32,
    },
    Dri3QueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    Dri3Open {
        sequence: u16,
    },
    Dri3GetSupportedModifiers {
        sequence: u16,
        window_modifiers: Vec<u64>,
        screen_modifiers: Vec<u64>,
    },
    /// `BufferFromPixmap`: the single-plane recovery of an imported pixmap.
    ///
    /// A separate record from `Dri3BuffersFromPixmap` because the wire replies
    /// are separate shapes, not one shape with a flag -- this one carries a
    /// total byte length and a single u16 stride where the other carries
    /// per-plane lists and a modifier.
    Dri3BufferFromPixmap {
        sequence: u16,
        size_bytes: u32,
        width: u16,
        height: u16,
        stride: u16,
        depth: u8,
        bits_per_pixel: u8,
    },
    /// `BuffersFromPixmap`: the modifier-aware, per-plane recovery.
    ///
    /// `strides` and `offsets` are the same length, and that length is the
    /// `nfd` the reply header promises. The descriptors themselves travel out
    /// of band rather than in this record.
    Dri3BuffersFromPixmap {
        sequence: u16,
        width: u16,
        height: u16,
        modifier: u64,
        depth: u8,
        bits_per_pixel: u8,
        strides: Vec<u32>,
        offsets: Vec<u32>,
    },
    XfixesQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    PresentQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    PresentQueryCapabilities {
        sequence: u16,
        capabilities: u32,
    },
    RandrQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    RandrGetScreenSizeRange {
        sequence: u16,
        min_width: u16,
        min_height: u16,
        max_width: u16,
        max_height: u16,
    },
    RandrGetScreenResources {
        sequence: u16,
        timestamp: u32,
        crtcs: Vec<u32>,
        outputs: Vec<u32>,
        modes: Vec<XRandrModeInfo>,
    },
    RandrGetOutputInfo {
        sequence: u16,
        timestamp: u32,
        crtc: u32,
        mm_width: u32,
        mm_height: u32,
        crtcs: Vec<u32>,
        modes: Vec<u32>,
        name: Vec<u8>,
    },
    RandrGetOutputProperty {
        sequence: u16,
        property_type: u32,
        bytes_after: u32,
        format: u8,
        data: Vec<u8>,
    },
    RandrGetCrtcInfo {
        sequence: u16,
        timestamp: u32,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        mode: u32,
        outputs: Vec<u32>,
    },
    RandrGetCrtcGammaSize {
        sequence: u16,
        size: u16,
    },
    RandrGetOutputPrimary {
        sequence: u16,
        output: u32,
    },
    RandrGetProviders {
        sequence: u16,
        timestamp: u32,
    },
    RandrGetMonitors {
        sequence: u16,
        timestamp: u32,
        monitors: Vec<XRandrMonitorInfo>,
    },
    XkbUseExtension {
        sequence: u16,
        supported: bool,
        server_major: u16,
        server_minor: u16,
    },
    GlxQueryVersion {
        sequence: u16,
        major_version: u32,
        minor_version: u32,
    },
    GlxString {
        sequence: u16,
        value: String,
    },
    GlxVisualConfigs {
        sequence: u16,
        configs: Vec<[u32; 18]>,
    },
    GlxFbConfigs {
        sequence: u16,
        configs: Vec<Vec<(u32, u32)>>,
    },
    GlxIsDirect {
        sequence: u16,
        direct: bool,
    },
    GlxMakeCurrent {
        sequence: u16,
        context_tag: u32,
    },
    GlxDrawableAttributes {
        sequence: u16,
        attributes: Vec<(u32, u32)>,
    },
    SyncInitialize {
        sequence: u16,
        major_version: u8,
        minor_version: u8,
    },
    SyncListSystemCounters {
        sequence: u16,
    },
    SyncQueryCounter {
        sequence: u16,
        value: i64,
    },
    XkbGetMap {
        sequence: u16,
        present: u16,
        keysyms: Vec<[u32; 2]>,
        modifier_map: Vec<(u8, u8)>,
    },
    XkbGetCompatMap {
        sequence: u16,
        device_id: u8,
    },
    XkbGetIndicatorMap {
        sequence: u16,
        device_id: u8,
    },
    XkbGetState {
        sequence: u16,
        modifiers: u8,
    },
    XkbGetControls {
        sequence: u16,
    },
    XkbGetNames {
        sequence: u16,
        which: u32,
        min_keycode: u8,
        max_keycode: u8,
        component_atoms: Vec<u32>,
        type_atoms: Vec<u32>,
        key_names: Vec<[u8; 4]>,
    },
    XkbGetDeviceInfo {
        sequence: u16,
        device_id: u8,
        supported: u16,
        unsupported: u16,
    },
    XkbPerClientFlags {
        sequence: u16,
        supported: u32,
        value: u32,
    },
    XiQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
    },
    GeQueryVersion {
        sequence: u16,
        major_version: u16,
        minor_version: u16,
    },
    XiGetClientPointer {
        sequence: u16,
        device_id: u16,
    },
    XiGetExtensionVersion {
        sequence: u16,
        server_major: u16,
        server_minor: u16,
    },
    XiQueryDevice {
        sequence: u16,
        devices: Vec<XXiDeviceInfo>,
    },
    XiListInputDevices {
        sequence: u16,
        devices: Vec<XXiLegacyDeviceInfo>,
    },
    XiQueryPointer {
        sequence: u16,
        root: XResourceId,
        child: XResourceId,
        root_x: i16,
        root_y: i16,
        win_x: i16,
        win_y: i16,
        buttons: u32,
        modifiers: u16,
    },
    XiGetFocus {
        sequence: u16,
        focus: XResourceId,
    },
    XiGetProperty {
        sequence: u16,
    },
    BigRequestsEnable {
        sequence: u16,
        maximum_request_length: u32,
    },
    GetInputFocus {
        sequence: u16,
        focus: XResourceId,
        revert_to: u8,
    },
    QueryPointer {
        sequence: u16,
        root: XResourceId,
        child: XResourceId,
        root_x: i16,
        root_y: i16,
        win_x: i16,
        win_y: i16,
        mask: u16,
    },
    GetModifierMapping {
        sequence: u16,
        keycodes_per_modifier: u8,
        keycodes: Vec<u8>,
    },
    GetPointerMapping {
        sequence: u16,
        mapping: Vec<u8>,
    },
    GetKeyboardMapping {
        sequence: u16,
        keysyms_per_keycode: u8,
        keysyms: Vec<u32>,
    },
    GetKeyboardControl {
        sequence: u16,
    },
    TranslateCoordinates {
        sequence: u16,
        same_screen: bool,
        child: Option<XResourceId>,
        dst_x: i16,
        dst_y: i16,
    },
    QueryFont {
        sequence: u16,
        font_ascent: i16,
        font_descent: i16,
    },
    GetProperty {
        sequence: u16,
        property_type: u32,
        format: u8,
        bytes_after: u32,
        item_count: u32,
        bytes: Vec<u8>,
    },
    GetSelectionOwner {
        sequence: u16,
        owner: Option<XResourceId>,
    },
    AllocNamedColor {
        sequence: u16,
        pixel: u32,
        exact: XColorRgb16,
        screen: XColorRgb16,
    },
    AllocColor {
        sequence: u16,
        pixel: u32,
        red: u16,
        green: u16,
        blue: u16,
    },
    ListProperties {
        sequence: u16,
        atoms: Vec<u32>,
    },
    QueryColors {
        sequence: u16,
        colors: Vec<XColorRgb16>,
    },
}

/// One device in an XI1 `ListInputDevices` reply.
///
/// Separate from `XXiDeviceInfo` because XI1 and XI2 describe a device
/// differently: XI1 names the type with an atom, reports a `DeviceUse`, and has
/// no vocabulary for scroll classes. Both are projected from one table in the
/// XI dispatcher, so the difference is a shape difference, not a second truth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XXiLegacyDeviceInfo {
    pub device_id: u8,
    pub device_type: u32,
    pub device_use: u8,
    pub name: String,
    pub classes: Vec<XXiLegacyDeviceClass>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XXiLegacyDeviceClass {
    Key { min_keycode: u8, max_keycode: u8 },
    Button { button_count: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XXiDeviceInfo {
    pub device_id: u16,
    pub device_type: u16,
    pub attachment: u16,
    pub name: String,
    pub classes: Vec<XXiDeviceClass>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XXiDeviceClass {
    Key {
        source_id: u16,
        keys: Vec<u32>,
    },
    Button {
        source_id: u16,
        button_count: u16,
    },
    Valuator {
        source_id: u16,
        number: u16,
        min: i64,
        max: i64,
        value: i64,
    },
    Scroll {
        source_id: u16,
        number: u16,
        scroll_type: u16,
        flags: u32,
        increment: i64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRandrModeInfo {
    pub id: u32,
    pub width: u16,
    pub height: u16,
    pub refresh_millihz: u32,
    pub name: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XRandrMonitorInfo {
    pub name: u32,
    pub primary: bool,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
    pub mm_width: u32,
    pub mm_height: u32,
    pub outputs: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XClientOutput {
    Error(XClientError),
    Event(XClientEvent),
    Reply(XClientReply),
}

pub fn encode_x_client_output(byte_order: XByteOrder, output: XClientOutput) -> Vec<u8> {
    match output {
        XClientOutput::Error(error) => encode_x_client_error(byte_order, error).to_vec(),
        XClientOutput::Event(event) => encode_x_client_event(byte_order, event).to_vec(),
        XClientOutput::Reply(reply) => encode_x_client_reply(byte_order, reply),
    }
}

pub fn encode_x_client_reply(byte_order: XByteOrder, reply: XClientReply) -> Vec<u8> {
    let reply = match encode_core_early_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    let reply = match encode_render_extension_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    let reply = match encode_randr_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    let reply = match encode_xkb_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    let reply = match encode_glx_sync_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    let reply = match encode_x_input_reply(byte_order, reply) {
        Ok(bytes) => return bytes,
        Err(reply) => reply,
    };
    match encode_core_late_reply(byte_order, reply) {
        Ok(bytes) => bytes,
        Err(_) => unreachable!("reply escaped its family encoder"),
    }
}
