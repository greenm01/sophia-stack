pub fn encode_x_client_error(
    byte_order: XByteOrder,
    error: XClientError,
) -> [u8; X_CLIENT_OUTPUT_RECORD_LEN] {
    let mut out = [0; X_CLIENT_OUTPUT_RECORD_LEN];
    out[0] = X_ERROR;
    out[1] = error.code.wire_code();
    put_u16(byte_order, &mut out[2..4], error.sequence);
    put_u32(byte_order, &mut out[4..8], error.resource_id);
    put_u16(byte_order, &mut out[8..10], error.minor_code);
    out[10] = error.major_code;
    out
}

pub fn encode_x_client_event(byte_order: XByteOrder, event: XClientEvent) -> Vec<u8> {
    let mut out = vec![0; X_CLIENT_OUTPUT_RECORD_LEN];
    match event {
        XClientEvent::Key {
            sequence,
            pressed,
            keycode,
            time,
            root,
            event,
            state,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                if pressed { X_KEY_PRESS } else { X_KEY_RELEASE },
                keycode,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], root);
            put_resource(byte_order, &mut out[12..16], event);
            put_resource(byte_order, &mut out[16..20], XResourceId::NONE);
            put_i16(byte_order, &mut out[20..22], 0);
            put_i16(byte_order, &mut out[22..24], 0);
            put_i16(byte_order, &mut out[24..26], 0);
            put_i16(byte_order, &mut out[26..28], 0);
            put_u16(byte_order, &mut out[28..30], state);
            out[30] = 1;
        }
        XClientEvent::Focus {
            sequence,
            focused,
            detail,
            event,
            mode,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                if focused { X_FOCUS_IN } else { X_FOCUS_OUT },
                detail,
                sequence,
            );
            put_resource(byte_order, &mut out[4..8], event);
            out[8] = mode;
        }
        XClientEvent::XkbStateNotify {
            sequence,
            time,
            modifiers,
            changed,
            keycode,
            event_type,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_KEYBOARD_FIRST_EVENT,
                2,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], time);
            out[8] = 3;
            out[9] = modifiers;
            out[10] = modifiers;
            out[16] = modifiers;
            out[18] = modifiers;
            put_u16(byte_order, &mut out[24..26], changed);
            out[26] = keycode;
            out[27] = event_type;
        }
        XClientEvent::PointerMotion {
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        } => write_pointer_event(
            byte_order,
            &mut out,
            X_MOTION_NOTIFY,
            0,
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        ),
        XClientEvent::PointerButton {
            sequence,
            pressed,
            button,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        } => write_pointer_event(
            byte_order,
            &mut out,
            if pressed {
                X_BUTTON_PRESS
            } else {
                X_BUTTON_RELEASE
            },
            button,
            sequence,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
        ),
        XClientEvent::PointerCrossing {
            sequence,
            entered,
            detail,
            time,
            root,
            event,
            root_x,
            root_y,
            event_x,
            event_y,
            state,
            mode,
            focus,
        } => {
            write_pointer_event(
                byte_order,
                &mut out,
                if entered { 7 } else { 8 },
                detail,
                sequence,
                time,
                root,
                event,
                root_x,
                root_y,
                event_x,
                event_y,
                state,
            );
            out[30] = mode;
            out[31] = 1 | (u8::from(focus) << 1);
        }
        XClientEvent::Expose {
            sequence,
            window,
            x,
            y,
            width,
            height,
            count,
        } => {
            write_event_header(byte_order, &mut out, X_EXPOSE, 0, sequence);
            put_resource(byte_order, &mut out[4..8], window);
            put_u16(byte_order, &mut out[8..10], x);
            put_u16(byte_order, &mut out[10..12], y);
            put_u16(byte_order, &mut out[12..14], width);
            put_u16(byte_order, &mut out[14..16], height);
            put_u16(byte_order, &mut out[16..18], count);
        }
        XClientEvent::VisibilityNotify {
            sequence,
            window,
            state,
        } => {
            write_event_header(byte_order, &mut out, X_VISIBILITY_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], window);
            out[8] = state;
        }
        XClientEvent::CreateNotify {
            sequence,
            parent,
            window,
            x,
            y,
            width,
            height,
            border_width,
            override_redirect,
        } => {
            write_event_header(byte_order, &mut out, 16, 0, sequence);
            put_resource(byte_order, &mut out[4..8], parent);
            put_resource(byte_order, &mut out[8..12], window);
            put_i16(byte_order, &mut out[12..14], x);
            put_i16(byte_order, &mut out[14..16], y);
            put_u16(byte_order, &mut out[16..18], width);
            put_u16(byte_order, &mut out[18..20], height);
            put_u16(byte_order, &mut out[20..22], border_width);
            out[22] = u8::from(override_redirect);
        }
        XClientEvent::MapNotify {
            sequence,
            event,
            window,
            override_redirect,
        } => {
            write_event_header(byte_order, &mut out, X_MAP_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], event);
            put_resource(byte_order, &mut out[8..12], window);
            out[12] = u8::from(override_redirect);
        }
        XClientEvent::UnmapNotify {
            sequence,
            event,
            window,
            from_configure,
        } => {
            write_event_header(byte_order, &mut out, X_UNMAP_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], event);
            put_resource(byte_order, &mut out[8..12], window);
            out[12] = u8::from(from_configure);
        }
        XClientEvent::ConfigureNotify {
            sequence,
            synthetic,
            event,
            window,
            above_sibling,
            x,
            y,
            width,
            height,
            border_width,
            override_redirect,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                X_CONFIGURE_NOTIFY | if synthetic { 0x80 } else { 0 },
                0,
                sequence,
            );
            put_resource(byte_order, &mut out[4..8], event);
            put_resource(byte_order, &mut out[8..12], window);
            put_u32(
                byte_order,
                &mut out[12..16],
                above_sibling.map(raw_xid).unwrap_or(0),
            );
            put_i16(byte_order, &mut out[16..18], x);
            put_i16(byte_order, &mut out[18..20], y);
            put_u16(byte_order, &mut out[20..22], width);
            put_u16(byte_order, &mut out[22..24], height);
            put_u16(byte_order, &mut out[24..26], border_width);
            out[26] = u8::from(override_redirect);
        }
        XClientEvent::PropertyNotify {
            sequence,
            window,
            atom,
            time,
            new_value,
        } => {
            write_event_header(byte_order, &mut out, X_PROPERTY_NOTIFY, 0, sequence);
            put_resource(byte_order, &mut out[4..8], window);
            put_u32(byte_order, &mut out[8..12], atom);
            put_u32(byte_order, &mut out[12..16], time);
            out[16] = if new_value { PROPERTY_NEW_VALUE } else { 1 };
        }
        XClientEvent::SelectionClear {
            sequence,
            time,
            owner,
            selection,
        } => {
            write_event_header(byte_order, &mut out, 29, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], owner);
            put_u32(byte_order, &mut out[12..16], selection);
        }
        XClientEvent::SelectionRequest {
            sequence,
            time,
            owner,
            requestor,
            selection,
            target,
            property,
        } => {
            write_event_header(byte_order, &mut out, 30, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], owner);
            put_resource(byte_order, &mut out[12..16], requestor);
            put_u32(byte_order, &mut out[16..20], selection);
            put_u32(byte_order, &mut out[20..24], target);
            put_u32(byte_order, &mut out[24..28], property);
        }
        XClientEvent::SelectionNotify {
            sequence,
            time,
            requestor,
            selection,
            target,
            property,
        } => {
            write_event_header(byte_order, &mut out, X_SELECTION_NOTIFY, 0, sequence);
            put_u32(byte_order, &mut out[4..8], time);
            put_resource(byte_order, &mut out[8..12], requestor);
            put_u32(byte_order, &mut out[12..16], selection);
            put_u32(byte_order, &mut out[16..20], target);
            put_u32(byte_order, &mut out[20..24], property);
        }
        XClientEvent::ClientMessage { sequence, bytes } => {
            out = bytes.to_vec();
            put_u16(byte_order, &mut out[2..4], sequence);
        }
        XClientEvent::ShmCompletion {
            sequence,
            drawable,
            segment,
            offset,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_MIT_SHM_FIRST_EVENT,
                0,
                sequence,
            );
            put_resource(byte_order, &mut out[4..8], drawable);
            put_u16(
                byte_order,
                &mut out[8..10],
                u16::from(crate::X_MIT_SHM_PUT_IMAGE_MINOR_OPCODE),
            );
            out[10] = crate::X_MIT_SHM_MAJOR_OPCODE;
            put_resource(byte_order, &mut out[12..16], segment);
            put_u32(byte_order, &mut out[16..20], offset);
        }
        XClientEvent::PresentConfigureNotify {
            sequence,
            event_id,
            window,
            x,
            y,
            width,
            height,
            pixmap_width,
            pixmap_height,
            pixmap_flags,
        } => {
            out.resize(40, 0);
            out[0] = 35;
            out[1] = crate::X_PRESENT_MAJOR_OPCODE;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u32(byte_order, &mut out[4..8], 2);
            put_u16(byte_order, &mut out[8..10], 0);
            put_resource(byte_order, &mut out[12..16], event_id);
            put_resource(byte_order, &mut out[16..20], window);
            put_i16(byte_order, &mut out[20..22], x);
            put_i16(byte_order, &mut out[22..24], y);
            put_u16(byte_order, &mut out[24..26], width);
            put_u16(byte_order, &mut out[26..28], height);
            put_i16(byte_order, &mut out[28..30], 0);
            put_i16(byte_order, &mut out[30..32], 0);
            put_u16(byte_order, &mut out[32..34], pixmap_width);
            put_u16(byte_order, &mut out[34..36], pixmap_height);
            put_u32(byte_order, &mut out[36..40], pixmap_flags);
        }
        XClientEvent::PresentCompleteNotify {
            sequence,
            event_id,
            window,
            serial,
            ust,
            msc,
            mode,
        } => {
            // XCB appends full_sequence to its in-memory event structure.
            // It is not part of the Present XGE wire record described by
            // present.xml.
            out.resize(40, 0);
            out[0] = 35;
            out[1] = crate::X_PRESENT_MAJOR_OPCODE;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u32(byte_order, &mut out[4..8], 2);
            put_u16(byte_order, &mut out[8..10], 1);
            out[10] = 0;
            out[11] = mode;
            put_resource(byte_order, &mut out[12..16], event_id);
            put_resource(byte_order, &mut out[16..20], window);
            put_u32(byte_order, &mut out[20..24], serial);
            put_u64(byte_order, &mut out[24..32], ust);
            put_u64(byte_order, &mut out[32..40], msc);
        }
        XClientEvent::PresentIdleNotify {
            sequence,
            event_id,
            window,
            serial,
            pixmap,
            idle_fence,
        } => {
            out[0] = 35;
            out[1] = crate::X_PRESENT_MAJOR_OPCODE;
            put_u16(byte_order, &mut out[2..4], sequence);
            put_u16(byte_order, &mut out[8..10], 2);
            put_resource(byte_order, &mut out[12..16], event_id);
            put_resource(byte_order, &mut out[16..20], window);
            put_u32(byte_order, &mut out[20..24], serial);
            put_resource(byte_order, &mut out[24..28], pixmap);
            put_resource(
                byte_order,
                &mut out[28..32],
                idle_fence.unwrap_or(XResourceId::NONE),
            );
        }
        XClientEvent::RandrScreenChange {
            sequence,
            timestamp,
            config_timestamp,
            root,
            request_window,
            width,
            height,
            mm_width,
            mm_height,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT,
                1,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_u32(byte_order, &mut out[8..12], config_timestamp);
            put_resource(byte_order, &mut out[12..16], root);
            put_resource(byte_order, &mut out[16..20], request_window);
            put_u16(byte_order, &mut out[20..22], 0);
            put_u16(byte_order, &mut out[22..24], 0);
            put_u16(byte_order, &mut out[24..26], width);
            put_u16(byte_order, &mut out[26..28], height);
            put_u16(byte_order, &mut out[28..30], mm_width);
            put_u16(byte_order, &mut out[30..32], mm_height);
        }
        XClientEvent::RandrCrtcChange {
            sequence,
            timestamp,
            window,
            crtc,
            mode,
            x,
            y,
            width,
            height,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                0,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_resource(byte_order, &mut out[8..12], window);
            put_u32(byte_order, &mut out[12..16], crtc);
            put_u32(byte_order, &mut out[16..20], mode);
            put_u16(byte_order, &mut out[20..22], 1);
            put_i16(byte_order, &mut out[24..26], x);
            put_i16(byte_order, &mut out[26..28], y);
            put_u16(byte_order, &mut out[28..30], width);
            put_u16(byte_order, &mut out[30..32], height);
        }
        XClientEvent::RandrOutputChange {
            sequence,
            timestamp,
            window,
            output,
            crtc,
            mode,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                1,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_u32(byte_order, &mut out[8..12], timestamp);
            put_resource(byte_order, &mut out[12..16], window);
            put_u32(byte_order, &mut out[16..20], output);
            put_u32(byte_order, &mut out[20..24], crtc);
            put_u32(byte_order, &mut out[24..28], mode);
            put_u16(byte_order, &mut out[28..30], 1);
            out[30] = 0;
            out[31] = 0;
        }
        XClientEvent::RandrResourceChange {
            sequence,
            timestamp,
            window,
        } => {
            write_event_header(
                byte_order,
                &mut out,
                crate::X_RANDR_FIRST_EVENT + 1,
                5,
                sequence,
            );
            put_u32(byte_order, &mut out[4..8], timestamp);
            put_resource(byte_order, &mut out[8..12], window);
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn write_pointer_event(
    byte_order: XByteOrder,
    out: &mut [u8],
    event_type: u8,
    detail: u8,
    sequence: u16,
    time: XTimestamp,
    root: XResourceId,
    event: XResourceId,
    root_x: i16,
    root_y: i16,
    event_x: i16,
    event_y: i16,
    state: u16,
) {
    write_event_header(byte_order, out, event_type, detail, sequence);
    put_u32(byte_order, &mut out[4..8], time);
    put_resource(byte_order, &mut out[8..12], root);
    put_resource(byte_order, &mut out[12..16], event);
    put_resource(byte_order, &mut out[16..20], XResourceId::NONE);
    put_i16(byte_order, &mut out[20..22], root_x);
    put_i16(byte_order, &mut out[22..24], root_y);
    put_i16(byte_order, &mut out[24..26], event_x);
    put_i16(byte_order, &mut out[26..28], event_y);
    put_u16(byte_order, &mut out[28..30], state);
    out[30] = 1;
}
