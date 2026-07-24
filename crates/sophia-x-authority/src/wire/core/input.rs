fn decode_get_input_focus(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GET_INPUT_FOCUS, X_GET_INPUT_FOCUS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GetInputFocus)
}

fn decode_set_input_focus(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_SET_INPUT_FOCUS, X_SET_INPUT_FOCUS_REQ_LEN, bytes.len())?;
    if bytes[1] > 2 {
        return Err(XWireParseError::InvalidValue(u32::from(bytes[1])));
    }
    Ok(XWireRequest::SetInputFocus {
        focus: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        revert_to: bytes[1],
        time: context.byte_order.u32(&bytes[8..12]),
    })
}

fn decode_get_modifier_mapping(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_MODIFIER_MAPPING,
        X_GET_MODIFIER_MAPPING_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetModifierMapping)
}

fn decode_get_keyboard_mapping(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(
        X_GET_KEYBOARD_MAPPING,
        X_GET_KEYBOARD_MAPPING_REQ_LEN,
        bytes.len(),
    )?;
    Ok(XWireRequest::GetKeyboardMapping {
        first_keycode: bytes[4],
        count: bytes[5],
    })
}

fn decode_grab_button(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_BUTTON, X_GRAB_BUTTON_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabButton {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        event_mask: context.byte_order.u16(&bytes[8..10]),
        button: bytes[20],
        modifiers: context.byte_order.u16(&bytes[22..24]),
        owner_events: bytes[1] != 0,
        pointer_mode: bytes[10],
        keyboard_mode: bytes[11],
    })
}

fn decode_grab_pointer(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_POINTER, X_GRAB_POINTER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabPointer {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        event_mask: context.byte_order.u16(&bytes[8..10]),
        owner_events: bytes[1] != 0,
        pointer_mode: bytes[10],
        keyboard_mode: bytes[11],
        time: context.byte_order.u32(&bytes[20..24]),
    })
}

fn decode_ungrab_pointer(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_POINTER, X_UNGRAB_POINTER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabPointer {
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_ungrab_button(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_BUTTON, X_UNGRAB_BUTTON_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabButton {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        button: bytes[1],
        modifiers: context.byte_order.u16(&bytes[8..10]),
    })
}

fn decode_grab_keyboard(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_KEYBOARD, X_GRAB_KEYBOARD_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabKeyboard {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        owner_events: bytes[1] != 0,
        time: context.byte_order.u32(&bytes[8..12]),
        pointer_mode: bytes[12],
        keyboard_mode: bytes[13],
    })
}

fn decode_ungrab_keyboard(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_KEYBOARD, X_UNGRAB_KEYBOARD_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabKeyboard {
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_grab_key(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_KEY, X_GRAB_KEY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabKey {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        modifiers: context.byte_order.u16(&bytes[8..10]),
        key: bytes[10],
        pointer_mode: bytes[11],
        keyboard_mode: bytes[12],
        owner_events: bytes[1] != 0,
    })
}

fn decode_ungrab_key(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_KEY, X_UNGRAB_KEY_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabKey {
        window: XResourceId::new(u64::from(context.byte_order.u32(&bytes[4..8])), 1),
        key: bytes[1],
        modifiers: context.byte_order.u16(&bytes[8..10]),
    })
}

fn decode_allow_events(
    context: XWireClientContext,
    bytes: &[u8],
) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_ALLOW_EVENTS, X_ALLOW_EVENTS_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::AllowEvents {
        mode: bytes[1],
        time: context.byte_order.u32(&bytes[4..8]),
    })
}

fn decode_grab_server(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_GRAB_SERVER, X_GRAB_SERVER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::GrabServer)
}

fn decode_ungrab_server(bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    require_exact_len(X_UNGRAB_SERVER, X_UNGRAB_SERVER_REQ_LEN, bytes.len())?;
    Ok(XWireRequest::UngrabServer)
}

