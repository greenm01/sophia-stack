#[derive(Clone, Debug)]
struct XAuthorityX11SmokeReport {
    configure_notify: usize,
    map_notify: usize,
    property_bytes: usize,
    errors: usize,
}

/// What the MIT-SHM 1.2 round trip proved.
///
/// `written` and `read_back` are the point: a descriptor the server handed
/// over is only evidence if the memory behind it is the memory the server
/// holds, and the only way to know is to write through one side and read the
/// other.
#[derive(Clone, Debug)]
struct XAuthorityShmFdSmokeReport {
    display: String,
    major_version: u16,
    minor_version: u16,
    created_bytes: usize,
    written: usize,
    read_back: usize,
    attached_fd_segments: usize,
    oversize_refused: bool,
    errors: usize,
}

#[derive(Clone, Debug)]
struct XAuthorityX11rbSmokeReport {
    display: String,
    window: u32,
    title_bytes: usize,
    configure_notify: usize,
    map_notify: usize,
    errors: usize,
}

#[derive(Clone, Debug)]
struct XAuthorityXdpyinfoSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    mentions_sophia: bool,
    mentions_root: bool,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    title_bytes: usize,
    title_match: bool,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibDrawingSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    draw_ops: usize,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityXlibPutImageSmokeReport {
    display: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    image_ops: usize,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityExternalProbeSmokeReport {
    display: String,
    outcome: String,
    status: i32,
    stdout_bytes: usize,
    stderr_bytes: usize,
    requests: usize,
    opcode_count: usize,
    opcodes: String,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
    cpu_buffers: usize,
    cpu_buffer_bytes: usize,
    nonzero_pixel_bytes: usize,
    ascii_marker_match: bool,
    first_error: Option<String>,
    #[cfg_attr(not(feature = "native-session"), allow(dead_code))]
    observed_transactions: Vec<SurfaceTransaction>,
    #[cfg_attr(not(feature = "native-session"), allow(dead_code))]
    observed_cpu_buffers: Vec<XAuthorityCpuBufferSnapshot>,
}

#[cfg(feature = "native-session")]
#[derive(Clone, Debug)]
pub(crate) struct XAuthorityTerminalRenderProof {
    pub display: String,
    pub requests: usize,
    pub transactions: usize,
    pub runtime_committed: u64,
    pub runtime_surfaces: u64,
    pub cpu_buffers: Vec<XAuthorityCpuBufferSnapshot>,
    pub authority_batches: Vec<AuthorityTransactionIntake>,
}

#[derive(Clone, Debug)]
struct XAuthorityPresentPixmapSmokeReport {
    display: String,
    extension_opcode: u8,
    transactions: usize,
    runtime_committed: u64,
    runtime_surfaces: u64,
}

#[derive(Clone, Debug)]
struct XAuthorityRuntimeSmokeReport {
    socket_path: std::path::PathBuf,
    surfaces: usize,
    transactions: usize,
    portal_prompts: usize,
    selection_artifacts: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct XAuthorityXtermInputSmokeReport {
    pub display: String,
    pub keys: usize,
    pub initial_generation: u64,
    pub final_generation: u64,
    pub initial_checksum: u64,
    pub final_checksum: u64,
    pub text_match: bool,
}

struct XtermInputResultFile {
    path: std::path::PathBuf,
}

#[derive(Clone, Debug)]
struct XAuthorityKittyInputSmokeReport {
    display: String,
    routed_keys: usize,
    present_before_input: usize,
    present_after_input: usize,
    text_match: bool,
}

#[derive(Clone, Debug)]
struct XAuthorityVkcubeAdmissionSmokeReport {
    display: String,
    intent_observed: bool,
    admission_delivered: bool,
    dma_bufs: usize,
    presents: usize,
    feedback: usize,
}

impl Drop for XtermInputResultFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct XAuthorityXtermTwoClientSmokeReport {
    pub display: String,
    pub clients: usize,
    pub routed_keys: usize,
    pub initial_generation: u64,
    pub final_generation: u64,
    pub initial_checksum: u64,
    pub final_checksum: u64,
}

#[derive(Clone, Copy, Debug)]
enum ExternalProbeDisplayMode {
    Argument(&'static str),
    Environment,
}

#[derive(Clone, Copy, Debug)]
enum ExternalProbePixelProof {
    None,
    Nonzero,
    Fixed6x13WhiteOnBlack(&'static [u8]),
}

#[derive(Clone, Copy, Debug)]
struct ExternalProbeSmokeSpec {
    command_name: &'static str,
    label: &'static str,
    binary: &'static str,
    display_mode: ExternalProbeDisplayMode,
    args: &'static [&'static str],
    display_base: u32,
    namespace: u64,
    require_transactions: bool,
    pixel_proof: ExternalProbePixelProof,
    allow_proof_kill_without_transactions: bool,
    allow_client_failure_without_x_error: bool,
    proof_timeout_secs: u64,
}

const EXTERNAL_PROBE_SMOKES: &[ExternalProbeSmokeSpec] = &[
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xclock-smoke",
        label: "xclock",
        binary: "xclock",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-analog", "-norender", "-update", "1"],
        display_base: 6600,
        namespace: 48,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xeyes-smoke",
        label: "xeyes",
        binary: "xeyes",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 6800,
        namespace: 49,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xwininfo-root-smoke",
        label: "xwininfo",
        binary: "xwininfo",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-root"],
        display_base: 6900,
        namespace: 50,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xprop-root-smoke",
        label: "xprop",
        binary: "xprop",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-root"],
        display_base: 7000,
        namespace: 51,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xsetroot-name-smoke",
        label: "xsetroot",
        binary: "xsetroot",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-name", "Sophia Root"],
        display_base: 7100,
        namespace: 52,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xlogo-smoke",
        label: "xlogo",
        binary: "xlogo",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 7200,
        namespace: 53,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xmessage-smoke",
        label: "xmessage",
        binary: "xmessage",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["Sophia"],
        display_base: 7300,
        namespace: 54,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xrandr-query-smoke",
        label: "xrandr",
        binary: "xrandr",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["--query"],
        display_base: 7400,
        namespace: 55,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xcalc-smoke",
        label: "xcalc",
        binary: "xcalc",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[],
        display_base: 7500,
        namespace: 56,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xterm-smoke",
        label: "xterm",
        binary: "xterm",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &["-geometry", "80x24", "-title", "Sophia xterm", "-e", "true"],
        display_base: 7600,
        namespace: 57,
        require_transactions: false,
        pixel_proof: ExternalProbePixelProof::None,
        allow_proof_kill_without_transactions: true,
        allow_client_failure_without_x_error: true,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-xterm-render-smoke",
        label: "xterm_render",
        binary: "xterm",
        display_mode: ExternalProbeDisplayMode::Argument("-display"),
        args: &[
            "-geometry",
            "40x8",
            "-title",
            "Sophia xterm",
            "-fn",
            "6x13",
            "-fg",
            "#ffffff",
            "-bg",
            "#000000",
            "-cr",
            "#ffffff",
            "-cm",
            "-dc",
            "-xrm",
            "*numColorRegisters: 2",
            "-xrm",
            "*cursorBlink:false",
            "-tn",
            "vt100",
            "-hold",
            "-e",
            "sh",
            "-c",
            "i=1; while [ \"$i\" -le 80 ]; do printf 'SophiaStream%03d\\n' \"$i\"; i=$((i + 1)); sleep 0.02; done; sleep 1",
        ],
        display_base: 7650,
        namespace: 59,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Fixed6x13WhiteOnBlack(b"SophiaStream080"),
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-zenity-smoke",
        label: "zenity",
        binary: "zenity",
        display_mode: ExternalProbeDisplayMode::Environment,
        args: &[
            "--entry",
            "--title",
            "Sophia zenity",
            "--text",
            "Sophia GTK probe",
        ],
        display_base: 7700,
        namespace: 58,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
    ExternalProbeSmokeSpec {
        command_name: "x-authority-firefox-smoke",
        label: "firefox",
        binary: "firefox",
        display_mode: ExternalProbeDisplayMode::Environment,
        args: &["--new-instance", "--no-remote", "about:blank"],
        display_base: 7800,
        namespace: 61,
        require_transactions: true,
        pixel_proof: ExternalProbePixelProof::Nonzero,
        allow_proof_kill_without_transactions: false,
        allow_client_failure_without_x_error: false,
        proof_timeout_secs: 8,
    },
];
