#![cfg(test)]

use std::io;

use super::nonblocking_atomic_cursor_commit_is_deferred;

#[test]
fn nonblocking_cursor_commit_defers_would_block_and_linux_ebusy() {
    assert!(nonblocking_atomic_cursor_commit_is_deferred(
        &io::Error::from(io::ErrorKind::WouldBlock)
    ));
    assert!(nonblocking_atomic_cursor_commit_is_deferred(
        &io::Error::from_raw_os_error(16)
    ));
    assert!(!nonblocking_atomic_cursor_commit_is_deferred(
        &io::Error::from_raw_os_error(22)
    ));
}
