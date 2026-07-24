fn dispatch_extension_version_request(
    context: XDispatchContext,
    request: XWireRequest,
    _runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
) -> Result<XDispatchResult, XWireRequest> {
    if !matches!(
        &request,
            XWireRequest::ShmQueryVersion
            | XWireRequest::Dri3QueryVersion { .. }
            | XWireRequest::XfixesQueryVersion { .. }
    ) {
        return Err(request);
    }
    Ok(match request {
                XWireRequest::ShmQueryVersion => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::ShmQueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 2,
                        shared_pixmaps: false,
                        pixmap_format: 0,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::Dri3QueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::Dri3QueryVersion {
                        sequence: context.sequence,
                        major_version: 1,
                        minor_version: 2,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::XfixesQueryVersion { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::XfixesQueryVersion {
                        sequence: context.sequence,
                        major_version: 6,
                        minor_version: 0,
                    })],
                    metadata_candidates: Vec::new(),
                },
        _ => unreachable!("request family checked before dispatch"),
    })
}
