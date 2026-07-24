fn dispatch_sync_request(
    context: XDispatchContext,
    request: XWireRequest,
    runtime: &mut XAuthorityRuntime,
    _atoms: &mut XAtomTable,
) -> Result<XDispatchResult, XWireRequest> {
    if !matches!(
        &request,
            XWireRequest::SyncInitialize { .. }
            | XWireRequest::SyncDestroyFence { .. }
    ) {
        return Err(request);
    }
    Ok(match request {
                XWireRequest::SyncInitialize { .. } => XDispatchResult {
                    response: None,
                    outputs: vec![XClientOutput::Reply(XClientReply::SyncInitialize {
                        sequence: context.sequence,
                        major_version: 3,
                        minor_version: 1,
                    })],
                    metadata_candidates: Vec::new(),
                },
                XWireRequest::SyncDestroyFence { fence } => {
                    let outputs = runtime
                        .destroy_dri3_fence(context.namespace, fence)
                        .err()
                        .map(|error| {
                            XClientOutput::Error(x_error_from_runtime(
                                error,
                                context.sequence,
                                context.major_opcode,
                                u32::try_from(fence.local.raw()).unwrap_or(0),
                            ))
                        })
                        .into_iter()
                        .collect();
                    XDispatchResult {
                        response: None,
                        outputs,
                        metadata_candidates: Vec::new(),
                    }
                }
        _ => unreachable!("request family checked before dispatch"),
    })
}
