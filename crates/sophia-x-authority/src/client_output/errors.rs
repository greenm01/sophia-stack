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

/// Turns a runtime refusal into the X error a client sees.
///
/// `minor_code` is the failing request's minor opcode, or `0` for a core
/// request, which has none. It is a parameter rather than a default because
/// this used to hardcode zero: every extension refusal that came through here
/// then claimed to be minor opcode 0, which in most extensions is
/// `QueryVersion` -- a request that usually takes no resource and so cannot
/// produce the error being reported. A live session refused nine Present
/// requests and the evidence named a request that could not have failed.
pub fn x_error_from_runtime(
    error: XAuthorityRuntimeError,
    sequence: u16,
    major_code: u8,
    minor_code: u16,
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
        minor_code,
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
        synthetic: false,
        time,
        requestor,
        selection,
        target,
        property: X_ATOM_NONE,
    }
}
