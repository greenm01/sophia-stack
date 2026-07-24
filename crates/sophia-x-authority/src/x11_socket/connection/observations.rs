fn x11_observed_request_stage(request: &crate::XWireRequest) -> X11ObservedRequestStage {
    match request {
        crate::XWireRequest::GlxQueryServerString { .. } => {
            X11ObservedRequestStage::GlxQueryServerString
        }
        crate::XWireRequest::GlxGetFbConfigs { .. } => X11ObservedRequestStage::GlxGetFbConfigs,
        crate::XWireRequest::GlxCreateContext { .. } => X11ObservedRequestStage::GlxCreateContext,
        crate::XWireRequest::GlxCreateWindow { .. } => X11ObservedRequestStage::GlxCreateWindow,
        crate::XWireRequest::Dri3PixmapFromBuffers { .. } => {
            X11ObservedRequestStage::Dri3PixmapFromBuffers
        }
        crate::XWireRequest::PresentPixmap { .. }
        | crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
            kind: crate::XAuthorityRequestKind::PresentPixmap { .. },
            ..
        }) => X11ObservedRequestStage::PresentPixmap,
        crate::XWireRequest::GetKeyboardMapping { .. } => X11ObservedRequestStage::KeyboardMapping,
        crate::XWireRequest::Authority(crate::XAuthorityRequestPacket {
            kind: crate::XAuthorityRequestKind::RequestSelection { .. },
            ..
        }) => X11ObservedRequestStage::SelectionRequest,
        _ => X11ObservedRequestStage::Other,
    }
}
