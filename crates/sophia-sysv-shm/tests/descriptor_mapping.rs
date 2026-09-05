use sophia_sysv_shm::{AccessError, DescriptorMapping, MAX_SEGMENT_BYTES};
use std::os::fd::AsFd;

#[test]
fn a_sealed_segment_round_trips_through_its_own_mapping() {
    let (mapping, _descriptor) = DescriptorMapping::create_sealed(4096).unwrap();
    assert_eq!(mapping.len(), 4096);
    assert!(mapping.is_writable());

    mapping.write_bytes(16, &[1, 2, 3, 4]).unwrap();
    assert_eq!(mapping.copy_bytes(16, 4).unwrap(), vec![1, 2, 3, 4]);
    // Untouched memory reads back as the zeroes the kernel gave us.
    assert_eq!(mapping.copy_bytes(0, 4).unwrap(), vec![0, 0, 0, 0]);
}

#[test]
fn a_sealed_segment_cannot_be_resized_by_whoever_holds_it() {
    // The seal is the reason a read of this mapping cannot become a SIGBUS: a
    // client handed this descriptor cannot shorten the file under the server.
    let (mapping, descriptor) = DescriptorMapping::create_sealed(8192).unwrap();
    // SAFETY: the descriptor is live and owned by this test.
    let shrunk = unsafe { libc::ftruncate(std::os::fd::AsRawFd::as_raw_fd(&descriptor), 16) };
    assert_ne!(shrunk, 0, "a sealed segment must refuse to shrink");
    assert_eq!(mapping.len(), 8192);
}

#[test]
fn a_read_only_mapping_refuses_a_write() {
    let (_writable, descriptor) = DescriptorMapping::create_sealed(4096).unwrap();
    let mapping = DescriptorMapping::map(descriptor.as_fd(), true).unwrap();
    assert!(!mapping.is_writable());
    assert_eq!(
        mapping.write_bytes(0, &[1]),
        Err(AccessError::ReadOnlySegment)
    );
    // Reading is still what it was attached for.
    assert_eq!(mapping.copy_bytes(0, 1).unwrap(), vec![0]);
}

#[test]
fn every_access_is_bounded_by_the_length_the_descriptor_reported() {
    // The bound is what stands in for a seal on a descriptor we did not
    // allocate, so it has to hold on both shapes of read and on the write.
    let (mapping, _descriptor) = DescriptorMapping::create_sealed(64).unwrap();
    assert_eq!(mapping.copy_bytes(60, 8), Err(AccessError::OutOfBounds));
    assert_eq!(mapping.copy_bytes(64, 1), Err(AccessError::OutOfBounds));
    assert_eq!(
        mapping.write_bytes(63, &[0, 0]),
        Err(AccessError::OutOfBounds)
    );
    assert_eq!(
        mapping.copy_bytes(usize::MAX, 1),
        Err(AccessError::RangeOverflow)
    );
    // Rows are bounded per row rather than only in total.
    assert_eq!(
        mapping.copy_rows(0, 16, 0, 16, 8),
        Err(AccessError::OutOfBounds)
    );
    assert_eq!(mapping.copy_rows(0, 16, 0, 16, 4).unwrap().len(), 64);
}

#[test]
fn a_segment_larger_than_the_bound_is_refused_rather_than_allocated() {
    // CreateSegment takes a CARD32, so the request can name four gigabytes.
    assert_eq!(
        DescriptorMapping::create_sealed(MAX_SEGMENT_BYTES + 1).err(),
        Some(AccessError::TooLarge)
    );
    assert_eq!(
        DescriptorMapping::create_sealed(0).err(),
        Some(AccessError::TooLarge)
    );
}
