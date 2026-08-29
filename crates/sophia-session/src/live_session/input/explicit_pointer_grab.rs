use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ExplicitPointerGrabControlReport {
    pub prepared: usize,
    pub activated: usize,
    pub released: usize,
    pub aborted: usize,
    pub rejected: usize,
}

fn explicit_pointer_grab_rejection(
    error: sophia_engine::ApplicationRouteLeaseError,
) -> sophia_x_authority::XAuthorityExplicitPointerGrabRejection {
    match error {
        sophia_engine::ApplicationRouteLeaseError::SeatAlreadyOwned => {
            sophia_x_authority::XAuthorityExplicitPointerGrabRejection::AlreadyOwned
        }
        sophia_engine::ApplicationRouteLeaseError::NoLease
        | sophia_engine::ApplicationRouteLeaseError::IdentityMismatch
        | sophia_engine::ApplicationRouteLeaseError::StaleAuthoritySession
        | sophia_engine::ApplicationRouteLeaseError::StaleControlEpoch
        | sophia_engine::ApplicationRouteLeaseError::StalePresentation => {
            sophia_x_authority::XAuthorityExplicitPointerGrabRejection::Stale
        }
        _ => sophia_x_authority::XAuthorityExplicitPointerGrabRejection::Invalid,
    }
}

fn presented_input_identity(
    surface: SurfaceId,
    projections: &[sophia_backend_live::LivePresentedInputProjection],
) -> Option<(sophia_protocol::OutputId, u64)> {
    projections.iter().find_map(|projection| {
        projection
            .layers
            .iter()
            .any(|layer| layer.surface == surface)
            .then_some((projection.output, projection.epoch))
    })
}

pub(super) fn drain_explicit_pointer_grab_controls(
    owner: &sophia_x_authority::XAuthorityExplicitPointerGrabOwner,
    state: &mut ApplicationRouteLeaseState,
    client_routes: &XAuthorityClientSurfaceRoutes,
    focus: &InputFocusState,
    projections: &[sophia_backend_live::LivePresentedInputProjection],
    seat: SeatId,
    now_msec: u64,
) -> Result<ExplicitPointerGrabControlReport, Box<dyn std::error::Error>> {
    let mut report = ExplicitPointerGrabControlReport::default();
    while let Ok(request) = owner.try_recv() {
        let admission = request.admission;
        let response = match request.kind {
            sophia_x_authority::XAuthorityExplicitPointerGrabRequestKind::Prepare {
                anchor,
                replaces,
            } => {
                let surface = match anchor {
                    sophia_x_authority::XAuthorityExplicitPointerGrabAnchor::Surface(surface) => {
                        (client_routes.admission_for_surface(surface) == Some(admission))
                            .then_some(surface)
                    }
                    sophia_x_authority::XAuthorityExplicitPointerGrabAnchor::AdmissionDefault => {
                        focus
                            .focused_surface(seat)
                            .filter(|surface| {
                                client_routes.admission_for_surface(*surface) == Some(admission)
                                    && presented_input_identity(*surface, projections).is_some()
                            })
                            .or_else(|| {
                                client_routes
                                    .surfaces_for_admission(admission)
                                    .into_iter()
                                    .find(|surface| {
                                        presented_input_identity(*surface, projections).is_some()
                                    })
                            })
                    }
                };
                let Some(surface) = surface else {
                    report.rejected = report.rejected.saturating_add(1);
                    owner.respond(
                        request.id,
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                            sophia_x_authority::XAuthorityExplicitPointerGrabRejection::NotViewable,
                        ),
                    )?;
                    continue;
                };
                let Some((output, presentation_epoch)) =
                    presented_input_identity(surface, projections)
                else {
                    report.rejected = report.rejected.saturating_add(1);
                    owner.respond(
                        request.id,
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                            sophia_x_authority::XAuthorityExplicitPointerGrabRejection::NotViewable,
                        ),
                    )?;
                    continue;
                };
                let candidate = ApplicationRouteLeaseCandidate {
                    seat,
                    origin: sophia_engine::ApplicationRouteLeaseOrigin::ExplicitPointer,
                    target_surface: surface,
                    admission: admission.client_id,
                    scope: ApplicationRouteScope {
                        profile: admission.namespace.profile,
                        authority: admission.namespace.id,
                    },
                    authority_session_epoch: admission.auth_provenance.session_generation,
                    output,
                    presentation_epoch,
                    initiating_device: None,
                    initiating_button: None,
                };
                let result = match replaces {
                    Some(identity) => state.replace_explicit_provisional(identity, candidate),
                    None => state.begin_provisional(candidate),
                };
                match result {
                    Ok(lease) => {
                        report.prepared = report.prepared.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Prepared(
                            lease.identity,
                        )
                    }
                    Err(error) => {
                        report.rejected = report.rejected.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                            explicit_pointer_grab_rejection(error),
                        )
                    }
                }
            }
            sophia_x_authority::XAuthorityExplicitPointerGrabRequestKind::Activate {
                identity,
            } => {
                let result = state
                    .lease(identity.seat)
                    .filter(|lease| {
                        lease.identity == identity
                            && lease.origin
                                == sophia_engine::ApplicationRouteLeaseOrigin::ExplicitPointer
                            && lease.admission == admission.client_id
                    })
                    .ok_or(sophia_engine::ApplicationRouteLeaseError::IdentityMismatch)
                    .and_then(|lease| {
                        state.confirm(
                            identity,
                            lease.target_surface,
                            admission.client_id,
                            admission.auth_provenance.session_generation,
                        )
                    });
                match result {
                    Ok(_) => {
                        report.activated = report.activated.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Activated
                    }
                    Err(error) => {
                        report.rejected = report.rejected.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                            explicit_pointer_grab_rejection(error),
                        )
                    }
                }
            }
            sophia_x_authority::XAuthorityExplicitPointerGrabRequestKind::BeginRelease {
                identity,
            } => match state.request_exact_release(identity, admission.client_id, now_msec) {
                Ok(_) => sophia_x_authority::XAuthorityExplicitPointerGrabResponse::ReleaseReady,
                Err(error) => {
                    report.rejected = report.rejected.saturating_add(1);
                    sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                        explicit_pointer_grab_rejection(error),
                    )
                }
            },
            sophia_x_authority::XAuthorityExplicitPointerGrabRequestKind::FinishRelease {
                identity,
            } => match state.acknowledge_release(identity, admission.client_id) {
                Ok(_) => {
                    report.released = report.released.saturating_add(1);
                    sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Released
                }
                Err(error) => {
                    report.rejected = report.rejected.saturating_add(1);
                    sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                        explicit_pointer_grab_rejection(error),
                    )
                }
            },
            sophia_x_authority::XAuthorityExplicitPointerGrabRequestKind::Abort { identity } => {
                match state.reject(identity) {
                    Ok(_) => {
                        report.aborted = report.aborted.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Aborted
                    }
                    Err(error) => {
                        report.rejected = report.rejected.saturating_add(1);
                        sophia_x_authority::XAuthorityExplicitPointerGrabResponse::Rejected(
                            explicit_pointer_grab_rejection(error),
                        )
                    }
                }
            }
        };
        owner.respond(request.id, response)?;
    }
    Ok(report)
}
