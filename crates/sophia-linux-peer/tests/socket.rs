use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::net::UnixStream;

#[test]
fn pidfd_is_cloexec_and_rejected_rights_do_not_leak_on_peek_or_truncation() {
    let (sender, receiver) = UnixStream::pair().unwrap();
    let pidfd = sophia_linux_peer::socket_peer_pidfd(receiver.as_fd()).unwrap();
    // SAFETY: F_GETFD inspects an owned descriptor and takes no output pointer.
    assert_ne!(
        unsafe { libc::fcntl(pidfd.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
        0
    );
    let count = || std::fs::read_dir("/proc/self/fd").unwrap().count();
    let baseline = count();
    for rights in [1, 128] {
        let mut bytes = [b'x'];
        let mut iov = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: 1,
        };
        let mut ancillary = [0_usize; 128];
        // SAFETY: zero initializes a valid empty msghdr; live buffers are assigned below.
        let mut header: libc::msghdr = unsafe { std::mem::zeroed() };
        header.msg_iov = &mut iov;
        header.msg_iovlen = 1;
        header.msg_control = ancillary.as_mut_ptr().cast();
        // SAFETY: control storage is aligned and larger than the computed cmsg size.
        unsafe {
            header.msg_controllen =
                libc::CMSG_SPACE((rights * std::mem::size_of::<i32>()) as u32) as usize;
            let item = libc::CMSG_FIRSTHDR(&header);
            (*item).cmsg_level = libc::SOL_SOCKET;
            (*item).cmsg_type = libc::SCM_RIGHTS;
            (*item).cmsg_len =
                libc::CMSG_LEN((rights * std::mem::size_of::<i32>()) as u32) as usize;
            for index in 0..rights {
                std::ptr::write_unaligned(
                    libc::CMSG_DATA(item).cast::<i32>().add(index),
                    pidfd.as_raw_fd(),
                );
            }
            assert_eq!(libc::sendmsg(sender.as_raw_fd(), &header, 0), 1);
        }
        for peek in [true, true, false] {
            assert_eq!(
                sophia_linux_peer::recv_plain(receiver.as_fd(), &mut [0], peek)
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
            assert_eq!(
                count(),
                baseline,
                "rights leaked (count={rights}, peek={peek})"
            );
        }
    }
}
