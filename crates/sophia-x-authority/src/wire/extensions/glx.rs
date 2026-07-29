fn decode_glx(context: XWireClientContext, bytes: &[u8]) -> Result<XWireRequest, XWireParseError> {
    let id = |offset: usize| {
        XResourceId::new(
            u64::from(context.byte_order.u32(&bytes[offset..offset + 4])),
            1,
        )
    };
    match bytes[1] {
        X_GLX_QUERY_VERSION_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::GlxQueryVersion {
                major_version: context.byte_order.u32(&bytes[4..8]),
                minor_version: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GLX_GET_VISUAL_CONFIGS_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetVisualConfigs {
                screen: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_GLX_QUERY_EXTENSIONS_STRING_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxQueryExtensionsString)
        }
        X_GLX_QUERY_SERVER_STRING_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 12, bytes.len())?;
            Ok(XWireRequest::GlxQueryServerString {
                name: context.byte_order.u32(&bytes[8..12]),
            })
        }
        X_GLX_GET_FB_CONFIGS_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetFbConfigs {
                screen: context.byte_order.u32(&bytes[4..8]),
            })
        }
        X_GLX_CLIENT_INFO_MINOR_OPCODE
        | X_GLX_SET_CLIENT_INFO_ARB_MINOR_OPCODE
        | X_GLX_SET_CLIENT_INFO_2_ARB_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 16, bytes.len())?;
            Ok(XWireRequest::GlxClientInfo)
        }
        X_GLX_CREATE_CONTEXT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 24, bytes.len())?;
            let context_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(context_id)?;
            let share = context.byte_order.u32(&bytes[16..20]);
            Ok(XWireRequest::GlxCreateContext {
                context: XResourceId::new(u64::from(context_id), 1),
                config: XGlxContextConfig::Visual(context.byte_order.u32(&bytes[8..12])),
                screen: context.byte_order.u32(&bytes[12..16]),
                share: (share != 0).then(|| XResourceId::new(u64::from(share), 1)),
                direct: bytes[20] != 0,
            })
        }
        X_GLX_DESTROY_CONTEXT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxDestroyContext { context: id(4) })
        }
        X_GLX_MAKE_CURRENT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 16, bytes.len())?;
            let drawable = context.byte_order.u32(&bytes[4..8]);
            let context_id = context.byte_order.u32(&bytes[8..12]);
            Ok(XWireRequest::GlxMakeCurrent {
                drawable: (drawable != 0)
                    .then(|| XResourceId::new(u64::from(drawable), 1)),
                context: (context_id != 0)
                    .then(|| XResourceId::new(u64::from(context_id), 1)),
                old_context_tag: context.byte_order.u32(&bytes[12..16]),
            })
        }
        X_GLX_IS_DIRECT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxIsDirect { context: id(4) })
        }
        X_GLX_CREATE_NEW_CONTEXT_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 28, bytes.len())?;
            let context_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(context_id)?;
            let share = context.byte_order.u32(&bytes[20..24]);
            Ok(XWireRequest::GlxCreateContext {
                context: XResourceId::new(u64::from(context_id), 1),
                config: XGlxContextConfig::FbConfig(context.byte_order.u32(&bytes[8..12])),
                screen: context.byte_order.u32(&bytes[12..16]),
                share: (share != 0).then(|| XResourceId::new(u64::from(share), 1)),
                direct: bytes[24] != 0,
            })
        }
        X_GLX_CREATE_CONTEXT_ATTRIBS_ARB_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 28, bytes.len())?;
            let count = context.byte_order.u32(&bytes[24..28]) as usize;
            require_exact_len(
                X_GLX_MAJOR_OPCODE,
                28usize.saturating_add(count.saturating_mul(8)),
                bytes.len(),
            )?;
            let context_id = context.byte_order.u32(&bytes[4..8]);
            context.validate_new_resource_id(context_id)?;
            let share = context.byte_order.u32(&bytes[16..20]);
            Ok(XWireRequest::GlxCreateContext {
                context: XResourceId::new(u64::from(context_id), 1),
                config: XGlxContextConfig::FbConfig(context.byte_order.u32(&bytes[8..12])),
                screen: context.byte_order.u32(&bytes[12..16]),
                share: (share != 0).then(|| XResourceId::new(u64::from(share), 1)),
                direct: bytes[20] != 0,
            })
        }
        X_GLX_CREATE_WINDOW_MINOR_OPCODE => {
            require_len(X_GLX_MAJOR_OPCODE, 24, bytes.len())?;
            let count = context.byte_order.u32(&bytes[20..24]) as usize;
            require_exact_len(
                X_GLX_MAJOR_OPCODE,
                24usize.saturating_add(count.saturating_mul(8)),
                bytes.len(),
            )?;
            let glx = context.byte_order.u32(&bytes[16..20]);
            context.validate_new_resource_id(glx)?;
            Ok(XWireRequest::GlxCreateWindow {
                screen: context.byte_order.u32(&bytes[4..8]),
                fbconfig: context.byte_order.u32(&bytes[8..12]),
                window: id(12),
                glx_window: XResourceId::new(u64::from(glx), 1),
            })
        }
        X_GLX_DELETE_WINDOW_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxDeleteWindow { glx_window: id(4) })
        }
        X_GLX_GET_DRAWABLE_ATTRIBUTES_MINOR_OPCODE => {
            require_exact_len(X_GLX_MAJOR_OPCODE, 8, bytes.len())?;
            Ok(XWireRequest::GlxGetDrawableAttributes { drawable: id(4) })
        }
        other => Err(XWireParseError::UnknownOpcode(other)),
    }
}
