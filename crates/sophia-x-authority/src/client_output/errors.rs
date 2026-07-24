pub fn x_error_from_wire_parse(
    error: &XWireParseError,
    sequence: u16,
    major_code: u8,
    minor_code: u16,
) -> XClientError {
    let code = match error {
        XWireParseError::Truncated { .. }
        | XWireParseError::InvalidLength { .. }
        | XWireParseError::TrailingBytes(_) => XErrorCode::BadLength,
        XWireParseError::UnknownOpcode(_) => XErrorCode::BadRequest,
        XWireParseError::InvalidPropertyMode(_)
        | XWireParseError::InvalidPropertyFormat(_)
        | XWireParseError::InvalidEventType(_)
        | XWireParseError::InvalidValue(_)
        | XWireParseError::PropertyValueTooLarge { .. } => XErrorCode::BadValue,
        XWireParseError::ResourceIdOutsideClientRange { .. } => XErrorCode::BadIdChoice,
    };

    XClientError {
        code,
        sequence,
        resource_id: 0,
        minor_code,
        major_code,
    }
}

pub fn x_error_from_runtime(
    error: XAuthorityRuntimeError,
    sequence: u16,
    major_code: u8,
    resource_id: u32,
) -> XClientError {
    let code = match error {
        XAuthorityRuntimeError::InvalidResource
        | XAuthorityRuntimeError::UnknownResource
        | XAuthorityRuntimeError::WrongResourceKind
        | XAuthorityRuntimeError::InvalidSurface => XErrorCode::BadWindow,
        XAuthorityRuntimeError::InvalidNamespace
        | XAuthorityRuntimeError::CrossNamespaceDenied
        | XAuthorityRuntimeError::StaleGeneration
        | XAuthorityRuntimeError::UnknownRequestorNamespace
        | XAuthorityRuntimeError::MissingSourceNamespace
        | XAuthorityRuntimeError::SameNamespace
        | XAuthorityRuntimeError::PortalRejected => XErrorCode::BadAccess,
        XAuthorityRuntimeError::UnknownSourceOwner => XErrorCode::BadAtom,
    };

    XClientError {
        code,
        sequence,
        resource_id,
        minor_code: 0,
        major_code,
    }
}

pub fn x_selection_failure_event(
    sequence: u16,
    time: XTimestamp,
    requestor: XResourceId,
    selection: u32,
    target: u32,
) -> XClientEvent {
    XClientEvent::SelectionNotify {
        sequence,
        time,
        requestor,
        selection,
        target,
        property: X_ATOM_NONE,
    }
}

