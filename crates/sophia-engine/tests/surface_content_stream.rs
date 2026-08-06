use sophia_engine::{SurfaceContentAdmission, SurfaceContentStream};
use sophia_protocol::{BufferSource, SurfaceId, SurfaceTransactionKey, TransactionId};

fn surface(raw: u32) -> SurfaceId {
    SurfaceId::new(raw, 1)
}

fn owner(transaction: u64, surface: u32, handle: u64) -> SurfaceTransactionKey {
    SurfaceTransactionKey {
        transaction: TransactionId::from_raw(transaction),
        surface: SurfaceId::new(surface, 1),
        target_buffer: BufferSource::CpuBuffer { handle },
    }
}

#[test]
fn unrelated_surfaces_run_while_owned_surface_content_waits() {
    let mut stream = SurfaceContentStream::default();
    let present = owner(10, 1, 100);
    stream.begin(present).unwrap();

    assert_eq!(
        stream.admit(20, [surface(2)], []).unwrap(),
        SurfaceContentAdmission::Ready(20)
    );
    assert_eq!(
        stream.admit(21, [surface(1)], []).unwrap(),
        SurfaceContentAdmission::Deferred { superseded: None }
    );
    assert_eq!(stream.finish(present).unwrap(), vec![21]);
}

#[test]
fn exact_owner_is_required_to_release_content() {
    let mut stream = SurfaceContentStream::default();
    let present = owner(10, 1, 100);
    stream.begin(present).unwrap();
    stream.admit(20, [surface(1)], []).unwrap();

    assert!(stream.finish(owner(11, 1, 100)).is_err());
    assert_eq!(stream.deferred_len(), 1);
    assert_eq!(stream.finish(present).unwrap(), vec![20]);
}

#[test]
fn multi_surface_work_waits_for_every_owner_and_preserves_fifo() {
    let mut stream = SurfaceContentStream::default();
    let left = owner(10, 1, 100);
    let right = owner(11, 2, 200);
    stream.begin(left).unwrap();
    stream.begin(right).unwrap();
    stream.admit(20, [surface(1), surface(2)], []).unwrap();
    stream.admit(21, [surface(1)], []).unwrap();

    assert!(stream.finish(left).unwrap().is_empty());
    assert_eq!(stream.finish(right).unwrap(), vec![20, 21]);
}

#[test]
fn removal_bypasses_owned_surface() {
    let mut stream = SurfaceContentStream::default();
    stream.begin(owner(10, 1, 100)).unwrap();
    assert_eq!(
        stream.admit(20, [surface(1)], [surface(1)]).unwrap(),
        SurfaceContentAdmission::Ready(20)
    );
}

#[test]
fn deferred_capacity_and_shutdown_are_bounded() {
    let mut stream = SurfaceContentStream::with_capacity(1);
    stream.begin(owner(10, 1, 100)).unwrap();
    stream.admit(20, [surface(1)], []).unwrap();
    assert!(stream.admit(21, [surface(1)], []).is_err());
    assert_eq!(stream.discard(), 1);
    assert_eq!(stream.active_len(), 0);
    assert_eq!(stream.deferred_len(), 0);
}

#[test]
fn replaceable_same_surface_content_retains_only_the_newest_candidate() {
    let mut stream = SurfaceContentStream::with_capacity(1);
    let present = owner(10, 1, 100);
    stream.begin(present).unwrap();
    assert_eq!(
        stream
            .admit_latest_deferred(20, [surface(1)], [], |_| true)
            .unwrap(),
        SurfaceContentAdmission::Deferred { superseded: None }
    );
    assert_eq!(
        stream
            .admit_latest_deferred(21, [surface(1)], [], |candidate| *candidate == 20)
            .unwrap(),
        SurfaceContentAdmission::Deferred {
            superseded: Some(20)
        }
    );

    assert_eq!(stream.deferred_len(), 1);
    assert_eq!(stream.supersessions(), 1);
    assert_eq!(stream.max_deferred_len(), 1);
    assert_eq!(stream.max_latest_deferred_per_surface(), 1);
    assert_eq!(stream.finish(present).unwrap(), vec![21]);
}

#[test]
fn latest_replacement_does_not_cross_same_surface_ordering_work() {
    let mut stream = SurfaceContentStream::default();
    let present = owner(10, 1, 100);
    stream.begin(present).unwrap();
    stream.admit(20, [surface(1)], []).unwrap();
    assert_eq!(
        stream
            .admit_latest_deferred(21, [surface(1)], [], |candidate| *candidate == 19)
            .unwrap(),
        SurfaceContentAdmission::Deferred { superseded: None }
    );

    assert_eq!(stream.supersessions(), 0);
    assert_eq!(stream.finish(present).unwrap(), vec![20, 21]);
}
