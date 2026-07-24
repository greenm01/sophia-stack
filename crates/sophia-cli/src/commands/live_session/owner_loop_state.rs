#[derive(Clone, Copy, Debug, Default)]
struct SessionLoopMetrics {
    batches: usize,
    transactions: usize,
    cpu_buffer_updates: usize,
    dma_buf_registrations_observed: usize,
    fence_registrations_observed: usize,
    present_submissions_observed: usize,
    cpu_compositions: usize,
    coalesced_batches: usize,
    backend_ticks: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
    physical_events: usize,
    physical_keys_routed: usize,
    physical_pointer_events: usize,
    physical_pointer_routed: usize,
    physical_pointer_buttons_routed: usize,
    session_ticks: usize,
    max_compose: Duration,
    protocol_error_count: usize,
    expected_protocol_error_count: usize,
    cursor_moves_coalesced: u64,
    cursor_max_motion_to_submit: Duration,
}

impl SessionLoopMetrics {
    fn new(initialize_empty_runtime: bool) -> Self {
        Self {
            cpu_compositions: usize::from(initialize_empty_runtime),
            ..Self::default()
        }
    }
}
