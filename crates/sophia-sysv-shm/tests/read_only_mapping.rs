use sophia_sysv_shm::{ReadOnlyMapping, write_bytes};

struct TestSegment(libc::c_int);

impl TestSegment {
    fn new(len: usize) -> Self {
        // SAFETY: IPC_PRIVATE creates a new segment and does not dereference
        // process memory.
        let id = unsafe { libc::shmget(libc::IPC_PRIVATE, len, libc::IPC_CREAT | 0o600) };
        assert!(id >= 0);
        Self(id)
    }
}

impl Drop for TestSegment {
    fn drop(&mut self) {
        // SAFETY: self owns this test-only SysV segment identifier.
        let _ = unsafe { libc::shmctl(self.0, libc::IPC_RMID, core::ptr::null_mut()) };
    }
}

#[test]
fn retained_mapping_copies_only_requested_rows() {
    let segment = TestSegment::new(48);
    let bytes = (0_u8..48).collect::<Vec<_>>();
    write_bytes(u32::try_from(segment.0).unwrap(), 0, &bytes).unwrap();
    let mapping = ReadOnlyMapping::attach(u32::try_from(segment.0).unwrap()).unwrap();

    assert_eq!(
        mapping.copy_rows(0, 16, 4, 8, 2).unwrap(),
        [4, 5, 6, 7, 8, 9, 10, 11, 20, 21, 22, 23, 24, 25, 26, 27]
    );
    assert_eq!(mapping.len(), 48);
}
