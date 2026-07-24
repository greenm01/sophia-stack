use sophia_x_authority::{
    X11DispatchObservation, X11ObservedRequestStage, XAuthorityObservedTransactionBatch,
    XClientError, XClientOutput, XDispatchResult, XErrorCode, XServerFrontendClientId,
    XWireClientResourceRange,
};

fn observation(outputs: Vec<XClientOutput>) -> X11DispatchObservation {
    X11DispatchObservation {
        client: XServerFrontendClientId::from_raw(1),
        resource_id_range: XWireClientResourceRange {
            base: 0x0020_0000,
            mask: 0x000f_ffff,
        },
        sequence: 1,
        major_opcode: 42,
        request_stage: X11ObservedRequestStage::Other,
        failure: None,
        result: XDispatchResult {
            response: None,
            outputs,
            metadata_candidates: Vec::new(),
        },
        cpu_buffer_update: None,
        received_fd_count: 0,
        received_fds: Vec::new(),
        dri3_pixmap_import: None,
        dri3_fence_import: None,
        present_submission: None,
        released_dma_bufs: Vec::new(),
        released_fences: Vec::new(),
        server_reply_fd_count: 0,
    }
}

fn error(sequence: u16, major_code: u8, resource_id: u32) -> XClientOutput {
    XClientOutput::Error(XClientError {
        code: XErrorCode::BadWindow,
        sequence,
        resource_id,
        minor_code: 0,
        major_code,
    })
}

#[test]
fn protocol_error_observations_are_reduced_and_bounded() {
    let outputs = (0..20)
        .map(|sequence| error(sequence, 42, 0xdead_beef))
        .collect();
    let batch =
        XAuthorityObservedTransactionBatch::from_dispatch_observation(&observation(outputs))
            .expect("protocol errors produce an observation batch");

    assert_eq!(batch.protocol_errors.len(), 16);
    assert_eq!(
        batch.protocol_errors[0].code,
        XErrorCode::BadWindow.wire_code()
    );
    assert_eq!(batch.protocol_errors[0].sequence, 0);
    assert_eq!(batch.protocol_errors[0].minor_code, 0);
    assert_eq!(batch.protocol_errors[0].major_code, 42);
}

#[test]
fn only_exact_window_zero_geometry_probes_are_expected() {
    let outputs = vec![
        error(1, 3, 0),
        error(2, 14, 0),
        error(3, 3, 1),
        error(4, 7, 0),
    ];
    let batch =
        XAuthorityObservedTransactionBatch::from_dispatch_observation(&observation(outputs))
            .expect("protocol errors produce an observation batch");

    assert_eq!(batch.expected_protocol_errors.len(), 2);
    assert_eq!(batch.protocol_errors.len(), 2);
}
