use super::*;
use crate::commands::live_session::PersistentLiveLayout;
use sophia_protocol::{SurfaceConstraints, TransactionId};

#[test]
fn renderer_residency_tracks_only_cpu_buffers_owned_by_admission_groups() {
    let surface = SurfaceId::new(62, 1);
    let transaction = TransactionId::from_raw(368);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let group = crate::commands::live_session::LiveAdmissionAuthorityGroup {
        transaction,
        transactions: vec![SurfaceTransaction {
            transaction,
            authority: sophia_protocol::AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: geometry,
            target_buffer: BufferSource::CpuBuffer { handle: 369 },
            damage: Region::single(geometry),
            readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        }],
        present_submissions: Vec::new(),
        software_present_submissions: Vec::new(),
        superseded: false,
    };
    let mut layout = PersistentLiveLayout::default();
    let mut handles = Vec::new();

    layout.pre_admission_groups.push_back(group.clone());
    layout.write_pending_cpu_buffer_handles(&mut handles);
    assert_eq!(handles, vec![369]);

    layout.pre_admission_groups.clear();
    layout.released_admission_groups.push_back(group);
    layout.write_pending_cpu_buffer_handles(&mut handles);
    assert_eq!(handles, vec![369]);

    layout.released_admission_groups.clear();
    layout.write_pending_cpu_buffer_handles(&mut handles);
    assert!(handles.is_empty());
}

#[test]
fn released_admission_pixels_wait_for_policy_assignment() {
    let surface = SurfaceId::new(63, 1);
    let transaction = TransactionId::from_raw(370);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    };
    let group = crate::commands::live_session::LiveAdmissionAuthorityGroup {
        transaction,
        transactions: vec![SurfaceTransaction {
            transaction,
            authority: sophia_protocol::AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: geometry,
            target_buffer: BufferSource::CpuBuffer { handle: 371 },
            damage: Region::single(geometry),
            readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        }],
        present_submissions: Vec::new(),
        software_present_submissions: Vec::new(),
        superseded: false,
    };
    let batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(372));
    let mut layout = PersistentLiveLayout::default();
    layout.unmanaged_surfaces.insert(surface);
    layout.released_admission_groups.push_back(group);

    let (_, released) = layout.projected_batch(&batch);
    assert!(released.is_empty());
    assert_eq!(layout.released_admission_groups.len(), 1);

    layout.unmanaged_surfaces.remove(&surface);
    let (_, released) = layout.projected_batch(&batch);
    assert_eq!(released.len(), 1);
    assert!(layout.released_admission_groups.is_empty());
}

#[test]
fn pre_admission_group_queue_fails_closed_at_its_fixed_capacity() {
    let surface = SurfaceId::new(8, 1);
    let geometry = Rect {
        x: 0,
        y: 0,
        width: 64,
        height: 64,
    };
    let mut batch =
        crate::commands::live_session::wm_update_coordinator_batch(TransactionId::from_raw(20));
    batch.surface_presentations.push(
        sophia_x_authority::XAuthoritySurfacePresentationObservation {
            surface,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            owner: None,
            mapped: false,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    );
    batch
        .presentation_intents
        .push(sophia_protocol::SurfacePresentationIntent {
            surface,
            kind: sophia_protocol::SurfacePresentationIntentKind::Request,
            role: sophia_protocol::SurfacePresentationRole::PolicyManaged,
            geometry,
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        });
    let mut layout = PersistentLiveLayout::default();
    let first_observation = layout.observe_authority_batch(&batch);
    assert!(!first_observation.admission_group_overflowed);

    let mut overflowed = false;
    for index in 0..=crate::commands::live_session::PRE_ADMISSION_GROUP_CAPACITY {
        let transaction = TransactionId::from_raw(u64::try_from(index + 21).unwrap());
        let mut present = crate::commands::live_session::wm_update_coordinator_batch(transaction);
        present.transactions.push(SurfaceTransaction {
            transaction,
            authority: sophia_protocol::AuthorityKind::SophiaX,
            surface,
            namespace: None,
            target_geometry: geometry,
            target_buffer: BufferSource::DmaBuf {
                handle: transaction.raw(),
            },
            damage: Region::single(geometry),
            readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
            timeout_msec: 250,
            previous_committed_generation: 0,
        });
        present
            .present_submissions
            .push(sophia_x_authority::XAuthorityPresentSubmission {
                transaction,
                surface,
                buffer: sophia_protocol::BufferHandle::from_raw(transaction.raw()),
                x_offset: 0,
                y_offset: 0,
                acquire_fence: None,
                idle_fence: None,
            });
        overflowed |= layout
            .observe_authority_batch(&present)
            .admission_group_overflowed;
    }

    assert!(overflowed);
    assert_eq!(
        layout.pre_admission_groups.len(),
        crate::commands::live_session::PRE_ADMISSION_GROUP_CAPACITY
    );
}
