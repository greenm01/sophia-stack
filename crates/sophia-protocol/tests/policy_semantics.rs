use sophia_protocol::*;

#[test]
fn complete_scene_snapshot_roundtrips_without_policy_private_state() {
    let scene = PolicySceneSnapshot {
        generation: 7,
        outputs: vec![PolicyOutputSnapshot {
            output: OutputId::from_raw(1),
            generation: 3,
            focus: Some(SurfaceId::new(3, 1)),
            bounds: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            work_area: Rect {
                x: 0,
                y: 24,
                width: 1920,
                height: 1056,
            },
        }],
        surfaces: vec![surface()],
        session_operations: vec![PolicySessionOperation {
            token: 11,
            slot: 1,
            permits_surface_target: true,
        }],
    };
    let bindings = vec![WmBindingRegistration {
        action: WmActionId::from_raw(4),
        keycode: 33,
        modifiers: WmModifierMask {
            bits: WmModifierMask::SUPER,
        },
    }];
    let transfer =
        encode_wm_v1_policy_snapshot(TransactionId::from_raw(9), 2, &scene, &bindings).unwrap();
    let decoded = decode_wm_v1_policy_snapshot(&transfer).unwrap();

    assert_eq!(decoded.scene, scene);
    assert_eq!(decoded.bindings, bindings);
    assert_eq!(transfer.chunks.len(), 4);
}

#[test]
fn complete_output_projection_roundtrips_in_stacking_order() {
    let proposal = PolicyProjectionProposal {
        transaction: TransactionId::from_raw(11),
        connection_epoch: 2,
        request_id: 5,
        base_generation: 7,
        outputs: vec![PolicyOutputProjection {
            output: OutputId::from_raw(1),
            placements: vec![PolicySurfacePlacement {
                surface: SurfaceId::new(3, 1),
                surface_generation: 8,
                geometry: Rect {
                    x: 20,
                    y: 30,
                    width: 800,
                    height: 600,
                },
                requested_size: Some(Size {
                    width: 800,
                    height: 600,
                }),
                crop: Some(Rect {
                    x: 1,
                    y: 2,
                    width: 799,
                    height: 598,
                }),
                transform: PolicyTransform::Identity,
                presentation: PolicyPresentationState {
                    fullscreen: true,
                    ..PolicyPresentationState::default()
                },
            }],
            focus: Some(SurfaceId::new(3, 1)),
        }],
    };
    let transfer = encode_wm_v1_policy_projection(&proposal).unwrap();

    assert_eq!(decode_wm_v1_policy_projection(&transfer), Ok(proposal));
    assert_eq!(transfer.chunks.len(), 2);
}

#[test]
fn projection_record_count_mismatch_fails_closed() {
    let proposal = PolicyProjectionProposal {
        transaction: TransactionId::from_raw(13),
        connection_epoch: 2,
        request_id: 5,
        base_generation: 7,
        outputs: vec![PolicyOutputProjection {
            output: OutputId::from_raw(1),
            placements: Vec::new(),
            focus: None,
        }],
    };
    let mut transfer = encode_wm_v1_policy_projection(&proposal).unwrap();
    transfer.begin.placement_count = 1;

    assert!(matches!(
        decode_wm_v1_policy_projection(&transfer),
        Err(IpcCodecError::InvalidEnum {
            field: "record_count",
            ..
        })
    ));
}

#[test]
fn projection_request_carries_the_complete_affected_output_set() {
    let request = PolicyProjectionRequest {
        connection_epoch: 2,
        request_id: 5,
        scene_generation: 7,
        affected_outputs: vec![OutputId::from_raw(1), OutputId::from_raw(2)],
        cause: PolicyRequestCause::Action {
            activation_serial: 9,
            action: WmActionId::from_raw(4),
        },
    };
    let encoded = encode_wm_v1_policy_projection_request(&request).unwrap();

    assert_eq!(
        decode_wm_v1_policy_projection_request(&encoded),
        Ok(request)
    );
    assert_eq!(encoded.affected_outputs.len(), 16);
}

#[test]
fn projection_request_rejects_duplicate_or_truncated_output_ids() {
    let duplicate = PolicyProjectionRequest {
        connection_epoch: 2,
        request_id: 5,
        scene_generation: 7,
        affected_outputs: vec![OutputId::from_raw(1), OutputId::from_raw(1)],
        cause: PolicyRequestCause::SceneChanged,
    };
    assert!(encode_wm_v1_policy_projection_request(&duplicate).is_err());

    let mut encoded = encode_wm_v1_policy_projection_request(&PolicyProjectionRequest {
        affected_outputs: vec![OutputId::from_raw(1)],
        ..duplicate
    })
    .unwrap();
    encoded.affected_outputs.pop();
    assert!(decode_wm_v1_policy_projection_request(&encoded).is_err());
}

#[test]
fn projection_outcome_has_one_strict_semantic_mapping() {
    for outcome in [
        PolicyProjectionOutcome::Committed,
        PolicyProjectionOutcome::RejectedStale,
        PolicyProjectionOutcome::RejectedInvalid,
        PolicyProjectionOutcome::TimedOut,
        PolicyProjectionOutcome::Disconnected,
    ] {
        let encoded = encode_wm_v1_policy_projection_outcome(2, 5, 7, outcome).unwrap();
        assert_eq!(
            decode_wm_v1_policy_projection_outcome(&encoded),
            Ok(outcome)
        );
    }
}

#[test]
fn configuration_dirty_and_session_operations_have_typed_mappings() {
    let configuration = PolicyConfiguration {
        connection_epoch: 2,
        generation: 3,
        bindings: vec![WmBindingRegistration {
            action: WmActionId::from_raw(7),
            keycode: 38,
            modifiers: WmModifierMask {
                bits: WmModifierMask::SUPER,
            },
        }],
        chrome: WmChromePolicy::default(),
    };
    let encoded = encode_wm_v1_policy_configuration(&configuration).unwrap();
    assert_eq!(
        decode_wm_v1_policy_configuration(&encoded),
        Ok(configuration)
    );

    let dirty = PolicyDirtyRequest {
        connection_epoch: 2,
        policy_generation: 4,
        affected_outputs: vec![OutputId::from_raw(1), OutputId::from_raw(2)],
    };
    let encoded = encode_wm_v1_policy_dirty(&dirty).unwrap();
    assert_eq!(decode_wm_v1_policy_dirty(&encoded), Ok(dirty));

    let operation = PolicySessionOperationRequest {
        connection_epoch: 2,
        request_id: 9,
        operation: 11,
        target: Some(SurfaceId::new(3, 1)),
    };
    let encoded = encode_wm_v1_policy_session_operation_request(operation).unwrap();
    assert_eq!(
        decode_wm_v1_policy_session_operation_request(&encoded),
        Ok(operation)
    );
}

#[test]
fn policy_configuration_rejects_ambiguous_or_reserved_bindings() {
    let binding = WmBindingRegistration {
        action: WmActionId::from_raw(7),
        keycode: 38,
        modifiers: WmModifierMask {
            bits: WmModifierMask::SUPER,
        },
    };
    let mut configuration = PolicyConfiguration {
        connection_epoch: 2,
        generation: 3,
        bindings: vec![binding, binding],
        chrome: WmChromePolicy::default(),
    };
    assert!(encode_wm_v1_policy_configuration(&configuration).is_err());

    configuration.bindings = vec![WmBindingRegistration {
        action: WmActionId::from_raw(8),
        keycode: 14,
        modifiers: WmModifierMask {
            bits: WmModifierMask::CONTROL | WmModifierMask::ALT,
        },
    }];
    assert!(encode_wm_v1_policy_configuration(&configuration).is_err());
}

#[test]
fn reduced_interaction_preserves_kind_phase_and_geometry() {
    let request = PolicyProjectionRequest {
        connection_epoch: 2,
        request_id: 6,
        scene_generation: 7,
        affected_outputs: vec![OutputId::from_raw(1)],
        cause: PolicyRequestCause::Interaction {
            phase: PolicyInteractionPhase::Cancel,
            kind: PolicyInteractionKind::Resize,
            target: SurfaceId::new(3, 1),
            geometry: Rect {
                x: 20,
                y: 30,
                width: 800,
                height: 600,
            },
        },
    };

    let encoded = encode_wm_v1_policy_projection_request(&request).unwrap();
    assert_eq!(
        decode_wm_v1_policy_projection_request(&encoded),
        Ok(request)
    );
}

fn surface() -> PolicySurfaceSnapshot {
    PolicySurfaceSnapshot {
        surface: SurfaceId::new(3, 1),
        generation: 8,
        current_output: Some(OutputId::from_raw(1)),
        kind: PolicySurfaceKind::Dialog,
        capabilities: LayoutNodeCapabilities::STANDARD_TOPLEVEL,
        constraints: SurfaceConstraints {
            min_size: Some(Size {
                width: 100,
                height: 80,
            }),
            max_size: None,
        },
        exact_size: None,
        requested_state: PolicyPresentationState {
            fullscreen: true,
            ..PolicyPresentationState::default()
        },
        current_state: PolicyPresentationState::default(),
        transient_owner: None,
        geometry: Rect {
            x: 20,
            y: 30,
            width: 800,
            height: 600,
        },
    }
}
