use super::*;

fn fixed_text_snapshot(handle: u64, rows: &[&[u8]]) -> XAuthorityCpuBufferSnapshot {
    let width = 120usize;
    let height = 52usize;
    let stride = width * 4;
    let mut bytes = vec![0; stride * height];
    for (line, text) in rows.iter().enumerate() {
        let top = line * 13;
        for (glyph_index, byte) in text.iter().copied().enumerate() {
            for (row, bits) in x_fixed_glyph_rows(byte).iter().copied().enumerate() {
                for column in 0..6 {
                    if bits & (1 << (5 - column)) == 0 {
                        continue;
                    }
                    let x = glyph_index * 6 + column;
                    let y = top + row;
                    let offset = y * stride + x * 4;
                    bytes[offset..offset + 4].copy_from_slice(&0x00ff_ffff_u32.to_le_bytes());
                }
            }
        }
    }
    XAuthorityCpuBufferSnapshot {
        handle,
        drawable: sophia_x_authority::XResourceId::new(0x220001, 1),
        size: sophia_protocol::Size {
            width: width as i32,
            height: height as i32,
        },
        stride: stride as u32,
        format: 0x3432_5258,
        generation: 1,
        bytes,
    }
}

fn transaction(surface: sophia_protocol::SurfaceId, handle: u64) -> SurfaceTransaction {
    SurfaceTransaction {
        transaction: sophia_protocol::TransactionId::from_raw(handle),
        authority: sophia_protocol::AuthorityKind::SophiaX,
        surface,
        namespace: None,
        target_geometry: sophia_protocol::Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 52,
        },
        content: sophia_protocol::SurfaceContentSet::singleton(sophia_protocol::BufferSource::CpuBuffer { handle }, sophia_protocol::Size {
            width: 120,
            height: 52,
        }),

        damage: sophia_protocol::Region::single(sophia_protocol::Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 52,
        }),
        readiness: sophia_protocol::SurfaceTransactionReadiness::Ready,
        timeout_msec: 250,
        previous_committed_generation: handle.saturating_sub(1),
    }
}

fn request(
    opcode: u8,
    surface: sophia_protocol::SurfaceId,
    handle: u64,
) -> ExternalProbeRequestProof {
    ExternalProbeRequestProof {
        opcode,
        transactions: vec![transaction(surface, handle)],
        removed_surfaces: Vec::new(),
        cpu_buffer_handle: Some(handle),
    }
}

#[test]
fn fixed_text_scroll_proof_requires_current_rows_and_causal_opcodes() {
    let rows: [&[u8]; 4] = [
        b"SophiaMirror09",
        b"SophiaMirror10",
        b"SophiaMirror11",
        b"SophiaMirror12",
    ];
    let surface = sophia_protocol::SurfaceId::new(1, 1);
    let current = fixed_text_snapshot(1, &rows);
    let buffers = [(1, current)].into_iter().collect();
    assert!(fixed_text_scroll_proof(
        &[request(76, surface, 1), request(62, surface, 1), request(76, surface, 1)],
        &buffers,
    ));
    assert!(!fixed_text_scroll_proof(
        &[request(62, surface, 1), request(76, surface, 1)],
        &buffers,
    ));
    assert!(!fixed_text_scroll_proof(
        &[request(76, surface, 1), request(62, surface, 1)],
        &buffers,
    ));
    let no_op_copy = ExternalProbeRequestProof {
        opcode: 62,
        transactions: Vec::new(),
        removed_surfaces: Vec::new(),
        cpu_buffer_handle: None,
    };
    assert!(!fixed_text_scroll_proof(
        &[request(76, surface, 1), no_op_copy, request(76, surface, 1)],
        &buffers,
    ));
}

#[test]
fn fixed_text_scroll_proof_rejects_a_stale_or_partial_surface() {
    let surface = sophia_protocol::SurfaceId::new(1, 1);
    let auxiliary = sophia_protocol::SurfaceId::new(2, 1);
    let stale = fixed_text_snapshot(1, &[
        b"SophiaMirror09",
        b"SophiaMirror10",
        b"SophiaMirror11",
        b"SophiaMirror12",
    ]);
    let current = fixed_text_snapshot(2, &[b"unrelated current surface"]);
    let partial = fixed_text_snapshot(3, &[b"SophiaMirror12"]);
    let reordered = fixed_text_snapshot(4, &[
        b"SophiaMirror09",
        b"SophiaMirror11",
        b"SophiaMirror10",
        b"SophiaMirror12",
    ]);

    assert!(cpu_buffer_contains_fixed_text(
        &stale,
        b"SophiaMirror12",
        Some((0x00ff_ffff, 0)),
    ));
    let causal = [
        request(76, surface, 1),
        request(62, surface, 1),
        request(76, surface, 1),
    ];
    let buffers = [(1, stale), (2, current.clone())].into_iter().collect();
    let mut with_auxiliary = causal.to_vec();
    with_auxiliary.push(request(65, auxiliary, 2));
    assert!(fixed_text_scroll_proof(&with_auxiliary, &buffers));

    let superseded = [causal.to_vec(), vec![request(65, surface, 2)]].concat();
    assert!(!fixed_text_scroll_proof(&superseded, &buffers));
    let partial_buffers = [(3, partial)].into_iter().collect();
    assert!(!fixed_text_scroll_proof(
        &[
            request(76, surface, 3),
            request(62, surface, 3),
            request(76, surface, 3),
        ],
        &partial_buffers,
    ));
    let reordered_buffers = [(4, reordered)].into_iter().collect();
    assert!(!fixed_text_scroll_proof(
        &[
            request(76, surface, 4),
            request(62, surface, 4),
            request(76, surface, 4),
        ],
        &reordered_buffers,
    ));
}
