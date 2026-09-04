use crate::XResourceId;
use sophia_protocol::{Rect, TransactionId};

/// Diagnostic-only CPU provenance. Never emit pixels or application metadata;
/// payload scans are opt-in and bounded by the validated image allocation cap.
pub(crate) fn trace_image_pixels(
    stage: &str,
    transaction: TransactionId,
    drawable: XResourceId,
    rect: Rect,
    bytes: &[u8],
) {
    if std::env::var("SOPHIA_X11_PIXEL_TRACE").as_deref() != Ok("1") {
        return;
    }
    let mut nonzero = 0usize;
    let mut checksum = 0xcbf29ce484222325u64;
    for pixel in bytes.chunks_exact(4) {
        nonzero += usize::from(pixel[..3] != [0, 0, 0]);
        for byte in &pixel[..3] {
            checksum = (checksum ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }
    tracing::info!(
        "sophia_x11_image_pixels schema=1 stage={stage} transaction={} drawable={} region={}x{}_{}_{} pixels={} nonzero_rgb_pixels={nonzero} checksum={checksum}",
        transaction.raw(),
        drawable.local.raw(),
        rect.width,
        rect.height,
        rect.x,
        rect.y,
        bytes.len() / 4,
    );
}
