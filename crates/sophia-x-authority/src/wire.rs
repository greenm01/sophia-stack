use sophia_protocol::{
    NamespaceId, PortalTransferId, Rect, Region, SurfaceConstraints, SurfaceId, TransactionId,
};

use crate::{
    XAtom, XAuthorityRequestKind, XAuthorityRequestPacket, XByteOrder, XClientEvent,
    XGraphicsContextValues, XPoint, XPropertyChange, XPropertyMode, XPropertyRead, XResourceId,
    XSelectionChangeKind, padded_len,
};

include!("wire/constants.rs");
include!("wire/core/color_cursor.rs");
include!("wire/core/drawing.rs");
include!("wire/core/discovery.rs");
include!("wire/core/input.rs");
include!("wire/core/properties.rs");
include!("wire/core/resources.rs");
include!("wire/core/windows.rs");
include!("wire/extensions/big_requests.rs");
include!("wire/extensions/dri3.rs");
include!("wire/extensions/glx.rs");
include!("wire/extensions/present.rs");
include!("wire/extensions/query_version.rs");
include!("wire/extensions/randr.rs");
include!("wire/extensions/shm.rs");
include!("wire/extensions/sophia_present.rs");
include!("wire/extensions/sync.rs");
include!("wire/extensions/xfixes.rs");
include!("wire/extensions/xi.rs");
include!("wire/extensions/xkb.rs");
include!("wire/validation.rs");

/// The XID range granted to one X11 client during connection setup.
///
/// Server-owned resources such as the root window are intentionally outside
/// this range. It therefore applies only when a request creates a new client
/// resource, not when it references an existing drawable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientResourceRange {
    pub base: u32,
    pub mask: u32,
}

impl XWireClientResourceRange {
    pub const fn owns_new_resource(self, resource_id: u32) -> bool {
        resource_id != 0 && (resource_id & !self.mask) == self.base
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct XWireClientContext {
    pub byte_order: XByteOrder,
    pub namespace: NamespaceId,
    pub transaction: TransactionId,
    /// `None` preserves deterministic decoder fixtures that are not attached
    /// to a live X11 setup. Socket clients must always provide their range.
    pub resource_id_range: Option<XWireClientResourceRange>,
}

impl XWireClientContext {
    fn validate_new_resource_id(self, resource_id: u32) -> Result<(), XWireParseError> {
        if self
            .resource_id_range
            .is_some_and(|range| !range.owns_new_resource(resource_id))
        {
            return Err(XWireParseError::ResourceIdOutsideClientRange { resource_id });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XGlxContextConfig {
    Visual(u32),
    FbConfig(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireRequest {
    Authority(XAuthorityRequestPacket),
    CreateWindow {
        packet: XAuthorityRequestPacket,
        parent: XResourceId,
        depth: u8,
        visual: u32,
        colormap: Option<XResourceId>,
        background_pixel: Option<u32>,
        override_redirect: bool,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    },
    ChangeWindowAttributes {
        window: XResourceId,
        override_redirect: Option<bool>,
        event_mask: Option<u32>,
        do_not_propagate_mask: Option<u32>,
    },
    GetWindowAttributes {
        window: XResourceId,
    },
    DestroyWindow {
        window: XResourceId,
    },
    ReparentWindow {
        window: XResourceId,
        parent: XResourceId,
        x: i16,
        y: i16,
    },
    MapSubwindows {
        window: XResourceId,
    },
    UnmapWindow {
        window: XResourceId,
    },
    ConfigureWindow {
        window: XResourceId,
        value_mask: u16,
        x: Option<i16>,
        y: Option<i16>,
        width: Option<u16>,
        height: Option<u16>,
        sibling: Option<XResourceId>,
        stack_mode: Option<u8>,
    },
    GetGeometry {
        drawable: XResourceId,
    },
    QueryTree {
        window: XResourceId,
    },
    InternAtom {
        only_if_exists: bool,
        name: String,
    },
    GetAtomName {
        atom: XAtom,
    },
    ChangeProperty(XPropertyChange),
    GetProperty(XPropertyRead),
    ListProperties {
        window: XResourceId,
    },
    GetSelectionOwner {
        selection: XAtom,
    },
    SendSelectionNotify {
        destination: XResourceId,
        event_mask: u32,
        event: XClientEvent,
    },
    GrabPointer {
        window: XResourceId,
        event_mask: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
        time: u32,
    },
    UngrabPointer {
        time: u32,
    },
    GrabButton {
        window: XResourceId,
        event_mask: u16,
        button: u8,
        modifiers: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
    },
    UngrabButton {
        window: XResourceId,
        button: u8,
        modifiers: u16,
    },
    GrabKeyboard {
        window: XResourceId,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
        time: u32,
    },
    UngrabKeyboard {
        time: u32,
    },
    GrabKey {
        window: XResourceId,
        key: u8,
        modifiers: u16,
        owner_events: bool,
        pointer_mode: u8,
        keyboard_mode: u8,
    },
    UngrabKey {
        window: XResourceId,
        key: u8,
        modifiers: u16,
    },
    AllowEvents {
        mode: u8,
        time: u32,
    },
    GrabServer,
    UngrabServer,
    CreateGraphicsContext {
        gc: XResourceId,
        drawable: XResourceId,
        values: XGraphicsContextValues,
    },
    ChangeGraphicsContext {
        gc: XResourceId,
        value_mask: u32,
        values: XGraphicsContextValues,
    },
    SetClipRectangles {
        gc: XResourceId,
        rectangles: Vec<Rect>,
    },
    FreeGraphicsContext {
        gc: XResourceId,
    },
    ClearArea {
        exposures: bool,
        window: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    },
    PolyFillRectangle {
        drawable: XResourceId,
        gc: XResourceId,
        rectangles: Vec<Rect>,
    },
    PutImage {
        format: u8,
        drawable: XResourceId,
        gc: XResourceId,
        width: u16,
        height: u16,
        dst_x: i16,
        dst_y: i16,
        left_pad: u8,
        depth: u8,
        data: Vec<u8>,
    },
    GetImage {
        format: u8,
        drawable: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
    },
    PolyText8 {
        drawable: XResourceId,
        gc: XResourceId,
        x: i16,
        y: i16,
        text: Vec<u8>,
    },
    ImageText8 {
        drawable: XResourceId,
        gc: XResourceId,
        x: i16,
        y: i16,
        text: Vec<u8>,
    },
    CreateColormap {
        alloc: u8,
        colormap: XResourceId,
        window: XResourceId,
        visual: u32,
    },
    FreeColormap {
        colormap: XResourceId,
    },
    AllocColor {
        colormap: XResourceId,
        red: u16,
        green: u16,
        blue: u16,
    },
    AllocNamedColor {
        colormap: XResourceId,
        name: String,
    },
    GetInputFocus,
    SetInputFocus {
        focus: XResourceId,
        revert_to: u8,
        time: u32,
    },
    OpenFont {
        font: XResourceId,
        name: String,
    },
    CloseFont {
        font: XResourceId,
    },
    QueryFont {
        font: XResourceId,
    },
    ListFonts {
        max_names: u16,
        pattern: String,
    },
    ListFontsWithInfo {
        max_names: u16,
        pattern: String,
    },
    CreatePixmap {
        depth: u8,
        pixmap: XResourceId,
        drawable: XResourceId,
        width: u16,
        height: u16,
    },
    FreePixmap {
        pixmap: XResourceId,
    },
    QueryExtension {
        name: String,
    },
    DeleteProperty {
        window: XResourceId,
        property: u32,
    },
    QueryPointer {
        window: XResourceId,
    },
    ListExtensions,
    QueryBestSize {
        class: u8,
        drawable: XResourceId,
        width: u16,
        height: u16,
    },
    CopyArea {
        source: XResourceId,
        destination: XResourceId,
        gc: XResourceId,
        src_x: i16,
        src_y: i16,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
    },
    PolySegment {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Vec<Rect>,
    },
    PolyLine {
        drawable: XResourceId,
        gc: XResourceId,
        points: Vec<XPoint>,
    },
    PolyRectangle {
        drawable: XResourceId,
        gc: XResourceId,
        rectangles: Vec<Rect>,
    },
    FillPoly {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Option<Rect>,
    },
    PolyFillArc {
        drawable: XResourceId,
        gc: XResourceId,
        damage: Vec<Rect>,
    },
    ShmQueryVersion,
    ShmGetImage {
        drawable: XResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
        plane_mask: u32,
        format: u8,
        segment: XResourceId,
        offset: u32,
    },
    Dri3QueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    Dri3Open {
        drawable: XResourceId,
        provider: u32,
    },
    Dri3PixmapFromBuffer {
        pixmap: XResourceId,
        drawable: XResourceId,
        size_bytes: u32,
        width: u16,
        height: u16,
        stride: u16,
        depth: u8,
        bits_per_pixel: u8,
    },
    Dri3PixmapFromBuffers {
        pixmap: XResourceId,
        window: XResourceId,
        num_buffers: u8,
        width: u16,
        height: u16,
        strides: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
        offsets: [u32; sophia_protocol::DMA_BUF_MAX_PLANES],
        depth: u8,
        bits_per_pixel: u8,
        modifier: u64,
    },
    Dri3FenceFromFd {
        drawable: XResourceId,
        fence: XResourceId,
        initially_triggered: bool,
    },
    Dri3GetSupportedModifiers {
        window: XResourceId,
        depth: u8,
        bits_per_pixel: u8,
    },
    XfixesQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    XfixesSelectSelectionInput {
        window: XResourceId,
        selection: XAtom,
        event_mask: u32,
    },
    XfixesCreateRegion {
        region: XResourceId,
        rectangles: Vec<Rect>,
    },
    XfixesDestroyRegion {
        region: XResourceId,
    },
    XfixesSetRegion {
        region: XResourceId,
        rectangles: Vec<Rect>,
    },
    PresentQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    PresentPixmap {
        transaction: TransactionId,
        window: XResourceId,
        pixmap: XResourceId,
        serial: u32,
        valid_region: u32,
        update_region: u32,
        x_offset: i16,
        y_offset: i16,
        target_crtc: u32,
        wait_fence: Option<XResourceId>,
        idle_fence: Option<XResourceId>,
        options: u32,
        target_msc: u64,
        divisor: u64,
        remainder: u64,
        notifies: Vec<(XResourceId, u32)>,
    },
    PresentSelectInput {
        event_id: XResourceId,
        window: XResourceId,
        event_mask: u32,
    },
    PresentQueryCapabilities {
        target: XResourceId,
    },
    ShmAttach {
        segment: XResourceId,
        shmid: u32,
        read_only: bool,
    },
    ShmDetach {
        segment: XResourceId,
    },
    ShmPutImage {
        drawable: XResourceId,
        gc: XResourceId,
        total_width: u16,
        total_height: u16,
        src_x: u16,
        src_y: u16,
        src_width: u16,
        src_height: u16,
        dst_x: i16,
        dst_y: i16,
        depth: u8,
        format: u8,
        send_event: bool,
        segment: XResourceId,
        offset: u32,
    },
    ShmCreatePixmap {
        pixmap: XResourceId,
        drawable: XResourceId,
        width: u16,
        height: u16,
        depth: u8,
        segment: XResourceId,
        offset: u32,
    },
    RandrQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    RandrSelectInput {
        window: XResourceId,
        enable: u16,
    },
    RandrGetScreenSizeRange {
        window: XResourceId,
    },
    RandrGetScreenResources {
        window: XResourceId,
        current: bool,
    },
    RandrGetOutputInfo {
        output: u32,
        config_timestamp: u32,
    },
    RandrGetOutputProperty {
        output: u32,
        property: XAtom,
        property_type: XAtom,
        long_offset: u32,
        long_length: u32,
        delete: bool,
        pending: bool,
    },
    RandrGetCrtcInfo {
        crtc: u32,
        config_timestamp: u32,
    },
    RandrGetCrtcGammaSize {
        crtc: u32,
    },
    RandrGetOutputPrimary {
        window: XResourceId,
    },
    RandrGetProviders {
        window: XResourceId,
    },
    RandrGetMonitors {
        window: XResourceId,
        get_active: bool,
    },
    XkbUseExtension {
        wanted_major: u16,
        wanted_minor: u16,
    },
    GlxQueryVersion {
        major_version: u32,
        minor_version: u32,
    },
    GlxGetVisualConfigs {
        screen: u32,
    },
    GlxGetFbConfigs {
        screen: u32,
    },
    GlxClientInfo,
    GlxCreateContext {
        context: XResourceId,
        config: XGlxContextConfig,
        screen: u32,
        share: Option<XResourceId>,
        direct: bool,
    },
    GlxDestroyContext {
        context: XResourceId,
    },
    GlxMakeCurrent {
        drawable: Option<XResourceId>,
        context: Option<XResourceId>,
        old_context_tag: u32,
    },
    GlxIsDirect {
        context: XResourceId,
    },
    GlxCreateWindow {
        screen: u32,
        fbconfig: u32,
        window: XResourceId,
        glx_window: XResourceId,
    },
    GlxDeleteWindow {
        glx_window: XResourceId,
    },
    GlxGetDrawableAttributes {
        drawable: XResourceId,
    },
    SyncInitialize {
        desired_major: u8,
        desired_minor: u8,
    },
    SyncListSystemCounters,
    SyncCreateCounter {
        counter: XResourceId,
        initial_value: i64,
    },
    SyncSetCounter {
        counter: XResourceId,
        value: i64,
    },
    SyncChangeCounter {
        counter: XResourceId,
        delta: i64,
    },
    SyncQueryCounter {
        counter: XResourceId,
    },
    SyncDestroyCounter {
        counter: XResourceId,
    },
    SyncDestroyFence {
        fence: XResourceId,
    },
    GlxQueryExtensionsString,
    GlxQueryServerString {
        name: u32,
    },
    XkbGetMap {
        full: u16,
        partial: u16,
    },
    XkbGetCompatMap {
        device_spec: u16,
    },
    XkbGetIndicatorMap {
        device_spec: u16,
    },
    XkbGetState,
    XkbGetControls,
    XkbGetNames {
        which: u32,
    },
    XkbGetDeviceInfo {
        device_spec: u16,
        wanted: u16,
    },
    XkbSelectEvents {
        affect_which: u16,
        clear: u16,
        select_all: u16,
        state_details: Option<(u16, u16)>,
    },
    XkbPerClientFlags {
        change: u32,
        value: u32,
    },
    XiQueryVersion {
        major_version: u16,
        minor_version: u16,
    },
    XiQueryPointer {
        window: XResourceId,
        device_id: u16,
    },
    XiGetClientPointer,
    XiDeviceBell,
    XiUngrabDevice {
        device_id: u16,
        time: u32,
    },
    XiChangeCursor {
        window: XResourceId,
        cursor: Option<XResourceId>,
    },
    XiGetExtensionVersion,
    XiQueryDevice {
        device_id: u16,
    },
    XiSelectEvents {
        window: XResourceId,
        masks: Vec<(u16, Vec<u32>)>,
    },
    XiGetFocus {
        device_id: u16,
    },
    XiGetProperty,
    GeQueryVersion {
        major_version: u16,
        minor_version: u16,
    },
    BigRequestsEnable,
    QueryColors {
        colormap: XResourceId,
        pixels: Vec<u32>,
    },
    CreateCursor {
        cursor: XResourceId,
        source: XResourceId,
        mask: Option<XResourceId>,
    },
    CreateGlyphCursor {
        cursor: XResourceId,
        source_font: XResourceId,
        mask_font: Option<XResourceId>,
    },
    FreeCursor {
        cursor: XResourceId,
    },
    RecolorCursor {
        cursor: XResourceId,
    },
    GetModifierMapping,
    GetPointerMapping,
    GetKeyboardMapping {
        first_keycode: u8,
        count: u8,
    },
    GetKeyboardControl,
    Bell,
    TranslateCoordinates {
        source: XResourceId,
        destination: XResourceId,
        src_x: i16,
        src_y: i16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XWireParseError {
    Truncated {
        needed: usize,
        actual: usize,
    },
    InvalidLength {
        opcode: u8,
        expected_at_least: usize,
        actual: usize,
    },
    TrailingBytes(usize),
    UnknownOpcode(u8),
    InvalidPropertyMode(u8),
    InvalidPropertyFormat(u8),
    InvalidEventType(u8),
    InvalidValue(u32),
    PropertyValueTooLarge {
        len: usize,
        max: usize,
    },
    ResourceIdOutsideClientRange {
        resource_id: u32,
    },
}

impl core::fmt::Display for XWireParseError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XWireParseError {}

pub fn decode_x11_core_request(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    if bytes.len() < 4 {
        return Err(XWireParseError::Truncated {
            needed: 4,
            actual: bytes.len(),
        });
    }

    let opcode = bytes[0];
    let declared_len = usize::from(context.byte_order.u16(&bytes[2..4])) * 4;
    if declared_len < 4 {
        return Err(XWireParseError::InvalidLength {
            opcode,
            expected_at_least: 4,
            actual: declared_len,
        });
    }
    if bytes.len() < declared_len {
        return Err(XWireParseError::Truncated {
            needed: declared_len,
            actual: bytes.len(),
        });
    }
    if bytes.len() > declared_len {
        return Err(XWireParseError::TrailingBytes(bytes.len() - declared_len));
    }

    match opcode {
        X_CREATE_WINDOW => decode_create_window(context, bytes),
        X_CHANGE_WINDOW_ATTRIBUTES => decode_change_window_attributes(context, bytes),
        X_GET_WINDOW_ATTRIBUTES => decode_get_window_attributes(context, bytes),
        X_DESTROY_WINDOW => decode_destroy_window(context, bytes),
        X_REPARENT_WINDOW => decode_reparent_window(context, bytes),
        X_MAP_WINDOW => decode_map_window(context, bytes),
        X_MAP_SUBWINDOWS => decode_map_subwindows(context, bytes),
        X_UNMAP_WINDOW => decode_unmap_window(context, bytes),
        X_CONFIGURE_WINDOW => decode_configure_window(context, bytes),
        X_GET_GEOMETRY => decode_get_geometry(context, bytes),
        X_QUERY_TREE => decode_query_tree(context, bytes),
        X_INTERN_ATOM => decode_intern_atom(context, bytes),
        X_GET_ATOM_NAME => decode_get_atom_name(context, bytes),
        X_CHANGE_PROPERTY => decode_change_property(context, bytes),
        X_DELETE_PROPERTY => {
            require_exact_len(X_DELETE_PROPERTY, X_DELETE_PROPERTY_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::DeleteProperty {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
                property: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GET_PROPERTY => decode_get_property(context, bytes),
        X_LIST_PROPERTIES => decode_list_properties(context, bytes),
        X_SET_SELECTION_OWNER => decode_set_selection_owner(context, bytes),
        X_GET_SELECTION_OWNER => decode_get_selection_owner(context, bytes),
        X_CONVERT_SELECTION => decode_convert_selection(context, bytes),
        X_SEND_EVENT => decode_send_event(context, bytes),
        X_GRAB_POINTER => decode_grab_pointer(context, bytes),
        X_UNGRAB_POINTER => decode_ungrab_pointer(context, bytes),
        X_GRAB_BUTTON => decode_grab_button(context, bytes),
        X_UNGRAB_BUTTON => decode_ungrab_button(context, bytes),
        X_GRAB_KEYBOARD => decode_grab_keyboard(context, bytes),
        X_UNGRAB_KEYBOARD => decode_ungrab_keyboard(context, bytes),
        X_GRAB_KEY => decode_grab_key(context, bytes),
        X_UNGRAB_KEY => decode_ungrab_key(context, bytes),
        X_ALLOW_EVENTS => decode_allow_events(context, bytes),
        X_GRAB_SERVER => decode_grab_server(bytes),
        X_UNGRAB_SERVER => decode_ungrab_server(bytes),
        X_TRANSLATE_COORDINATES => decode_translate_coordinates(context, bytes),
        X_QUERY_POINTER => {
            require_exact_len(X_QUERY_POINTER, X_QUERY_POINTER_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::QueryPointer {
                window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_SET_INPUT_FOCUS => decode_set_input_focus(context, bytes),
        X_GET_INPUT_FOCUS => decode_get_input_focus(bytes),
        X_GET_KEYBOARD_CONTROL => {
            require_exact_len(X_GET_KEYBOARD_CONTROL, 4, bytes.len())?;
            Ok(XWireRequest::GetKeyboardControl)
        }
        X_BELL => {
            require_exact_len(X_BELL, 4, bytes.len())?;
            Ok(XWireRequest::Bell)
        }
        X_OPEN_FONT => decode_open_font(context, bytes),
        X_CLOSE_FONT => decode_close_font(context, bytes),
        X_QUERY_FONT => decode_query_font(context, bytes),
        X_LIST_FONTS => decode_list_fonts(context, bytes),
        X_LIST_FONTS_WITH_INFO => decode_list_fonts_with_info(context, bytes),
        X_CREATE_PIXMAP => decode_create_pixmap(context, bytes),
        X_FREE_PIXMAP => decode_free_pixmap(context, bytes),
        X_CREATE_GC => decode_create_gc(context, bytes),
        X_SET_CLIP_RECTANGLES => decode_set_clip_rectangles(context, bytes),
        X_CHANGE_GC => decode_change_gc(context, bytes),
        X_FREE_GC => decode_free_gc(context, bytes),
        X_CLEAR_AREA => decode_clear_area(context, bytes),
        X_COPY_AREA => decode_copy_area(context, bytes),
        X_POLY_LINE => decode_poly_line(context, bytes),
        X_POLY_SEGMENT => decode_poly_segment(context, bytes),
        X_POLY_RECTANGLE => decode_poly_rectangle(context, bytes),
        X_FILL_POLY => decode_fill_poly(context, bytes),
        X_POLY_FILL_RECTANGLE => decode_poly_fill_rectangle(context, bytes),
        X_POLY_FILL_ARC => decode_poly_fill_arc(context, bytes),
        X_PUT_IMAGE => decode_put_image(context, bytes),
        X_GET_IMAGE => decode_get_image(context, bytes),
        X_POLY_TEXT8 => decode_poly_text8(context, bytes),
        X_IMAGE_TEXT8 => decode_image_text8(context, bytes),
        X_CREATE_COLORMAP => decode_create_colormap(context, bytes),
        X_FREE_COLORMAP => {
            require_exact_len(X_FREE_COLORMAP, X_FREE_COLORMAP_REQ_LEN, bytes.len())?;
            Ok(XWireRequest::FreeColormap {
                colormap: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
            })
        }
        X_ALLOC_COLOR => decode_alloc_color(context, bytes),
        X_ALLOC_NAMED_COLOR => decode_alloc_named_color(context, bytes),
        X_QUERY_COLORS => decode_query_colors(context, bytes),
        X_CREATE_CURSOR => decode_create_cursor(context, bytes),
        X_CREATE_GLYPH_CURSOR => decode_create_glyph_cursor(context, bytes),
        X_FREE_CURSOR => decode_free_cursor(context, bytes),
        X_RECOLOR_CURSOR => decode_recolor_cursor(context, bytes),
        X_QUERY_BEST_SIZE => decode_query_best_size(context, bytes),
        X_QUERY_EXTENSION => decode_query_extension(context, bytes),
        X_LIST_EXTENSIONS => decode_list_extensions(bytes),
        X_GET_KEYBOARD_MAPPING => decode_get_keyboard_mapping(bytes),
        X_GET_POINTER_MAPPING => decode_get_pointer_mapping(bytes),
        X_GET_MODIFIER_MAPPING => decode_get_modifier_mapping(bytes),
        X_SOPHIA_PRESENT_MAJOR_OPCODE => decode_sophia_present(context, bytes),
        X_MIT_SHM_MAJOR_OPCODE => decode_mit_shm(context, bytes),
        X_RANDR_MAJOR_OPCODE => decode_randr(context, bytes),
        X_KEYBOARD_MAJOR_OPCODE => decode_x_keyboard(context, bytes),
        X_BIG_REQUESTS_MAJOR_OPCODE => decode_big_requests(bytes),
        X_INPUT_MAJOR_OPCODE => decode_x_input(context, bytes),
        X_GENERIC_EVENT_MAJOR_OPCODE => {
            require_exact_len(
                X_GENERIC_EVENT_MAJOR_OPCODE,
                X_GENERIC_EVENT_QUERY_VERSION_REQ_LEN,
                bytes.len(),
            )?;
            if bytes[1] != X_GENERIC_EVENT_QUERY_VERSION_MINOR_OPCODE {
                return Err(XWireParseError::UnknownOpcode(bytes[1]));
            }
            Ok(XWireRequest::GeQueryVersion {
                major_version: context.byte_order.u16(&bytes[4..6]),
                minor_version: context.byte_order.u16(&bytes[6..8]),
            })
        }
        X_DRI3_MAJOR_OPCODE => decode_dri3(context, bytes),
        X_PRESENT_MAJOR_OPCODE => decode_present(context, bytes),
        X_XFIXES_MAJOR_OPCODE => decode_xfixes(context, bytes),
        X_GLX_MAJOR_OPCODE => decode_glx(context, bytes),
        X_SYNC_MAJOR_OPCODE => decode_sync(context, bytes),
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}
