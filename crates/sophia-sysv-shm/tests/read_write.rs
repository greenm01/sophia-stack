#[test]
fn bounded_write_round_trips_through_the_safe_adapter() {
    // SAFETY: this test allocates a private segment and removes the same ID
    // before returning.
    let shmid = unsafe { libc::shmget(libc::IPC_PRIVATE, 32, libc::IPC_CREAT | 0o600) };
    assert!(shmid >= 0);
    let shmid_u32 = u32::try_from(shmid).unwrap();

    sophia_sysv_shm::write_bytes(shmid_u32, 7, b"sophia").unwrap();
    assert_eq!(
        sophia_sysv_shm::copy_bytes(shmid_u32, 7, 6).unwrap(),
        b"sophia"
    );
    assert_eq!(
        sophia_sysv_shm::write_bytes(shmid_u32, 30, b"wide"),
        Err(sophia_sysv_shm::WriteError::OutOfBounds)
    );

    // SAFETY: shmid was allocated by this test and is no longer needed.
    assert_eq!(
        unsafe { libc::shmctl(shmid, libc::IPC_RMID, core::ptr::null_mut()) },
        0
    );
}
