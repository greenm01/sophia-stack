#[test]
fn native_frame_slot_pool_defers_the_fourth_live_generation() {
    let mut pool = LiveRendererFrameSlotPool::new();
    let tokens = std::array::from_fn(|_| match pool.try_acquire() {
        LiveRendererFrameSlotAcquire::Acquired(token) => token,
        other => panic!("expected a free native frame slot, got {other:?}"),
    });

    assert_eq!(pool.leased(), 3);
    assert_eq!(pool.try_acquire(), LiveRendererFrameSlotAcquire::Deferred);
    assert_eq!(pool.leased(), 3);
    assert_eq!(pool.metrics().acquisitions, 3);
    assert_eq!(pool.metrics().deferrals, 1);
    assert_eq!(pool.metrics().high_watermark, 3);
    assert_eq!(tokens.map(|token| token.slot_id().index()), [0, 1, 2]);
}

#[test]
fn native_frame_slot_reuse_advances_its_incarnation() {
    let mut pool = LiveRendererFrameSlotPool::new();
    let first = match pool.try_acquire() {
        LiveRendererFrameSlotAcquire::Acquired(token) => token,
        other => panic!("expected the first native frame slot, got {other:?}"),
    };

    assert_eq!(pool.release(first), LiveRendererFrameSlotRelease::Released);
    let second = match pool.try_acquire() {
        LiveRendererFrameSlotAcquire::Acquired(token) => token,
        other => panic!("expected a recycled native frame slot, got {other:?}"),
    };

    assert_ne!(second.slot_id(), first.slot_id());
    assert_eq!(second.incarnation(), 1);
    let mut recycled = None;
    for _ in 0..2 {
        let token = match pool.try_acquire() {
            LiveRendererFrameSlotAcquire::Acquired(token) => token,
            other => panic!("expected a remaining native frame slot, got {other:?}"),
        };
        if token.slot_id() == first.slot_id() {
            recycled = Some(token);
        }
    }
    let recycled = recycled.expect("round-robin allocation returns the released slot");
    assert_eq!(recycled.incarnation(), first.incarnation() + 1);
    assert_eq!(pool.metrics().reuses, 1);
}

#[test]
fn stale_native_frame_slot_release_cannot_free_a_reused_slot() {
    let mut pool = LiveRendererFrameSlotPool::new();
    let first = match pool.try_acquire() {
        LiveRendererFrameSlotAcquire::Acquired(token) => token,
        other => panic!("expected the first native frame slot, got {other:?}"),
    };
    assert_eq!(pool.release(first), LiveRendererFrameSlotRelease::Released);

    let mut current = None;
    for _ in 0..3 {
        let token = match pool.try_acquire() {
            LiveRendererFrameSlotAcquire::Acquired(token) => token,
            other => panic!("expected a native frame slot, got {other:?}"),
        };
        if token.slot_id() == first.slot_id() {
            current = Some(token);
        }
    }
    let current = current.expect("released slot must be reused after one round");
    assert_eq!(pool.leased(), 3);
    assert_eq!(
        pool.release(first),
        LiveRendererFrameSlotRelease::Stale,
        "an old incarnation must not release the current owner"
    );
    assert_eq!(pool.leased(), 3);
    assert_eq!(pool.metrics().stale_releases, 1);
    assert_eq!(pool.release(current), LiveRendererFrameSlotRelease::Released);
    assert_eq!(pool.leased(), 2);
}
