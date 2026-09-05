# Join Engine surface geometry -> head scene -> final region readback -> exact
# queued frame -> native retirement. Metadata is readiness, never pixel proof.
# Keys carry opaque IDs only. No fixed monitor size, layout position, or XID.
function fail(message) {
    print "Firefox rendering canary verification failed: " message > "/dev/stderr"
    failed = 1
    exit 1
}
function positive(value) { return value ~ /^[1-9][0-9]*$/ }
function scene_key() { return owner_epoch SUBSEP f["output"] SUBSEP f["head"] SUBSEP f["scene_generation"] }
function frame_key() { return owner_epoch SUBSEP f["output"] SUBSEP f["head"] SUBSEP f["frame"] }
function action_launch() {
    # The current emitter writes schema 2, then one schema-1 compatibility
    # echo. Consume only that echo; another launch must still fail the gate.
    if (f["schema"] == 1 && launch_echo && f["id"] == launch_id && observed < launch) {
        launch_echo = 0
        return
    }
    if (f["schema"] != 1 && f["schema"] != 2) fail("unsupported action launch schema")
    if (f["schema"] == 2 && !positive(f["transaction"])) fail("invalid action launch transaction")
    launches++
    launch = NR
    launch_id = f["id"]
    launch_echo = f["schema"] == 2
    launch_transaction = launch_echo ? "transaction:" f["transaction"] : ""
}
{
    # Accept production tracing prefixes, but only parse stable schema records.
    if ($0 ~ /(^Error:|panicked at|status=(failed|degraded|timed_out|hard_stall|head_lost)([[:space:]]|$))/)
        fail("fatal, degraded, or timed-out session")
    sub(/^.*sophia_/, "sophia_")
    if ($1 !~ /^sophia_/) next
    delete f
    for (i = 2; i <= NF; i++) {
        split($i, pair, "=")
        f[pair[1]] = pair[2]
    }
    event = $1
    status = f["status"]
    # Frame and scene IDs are local to a replaceable native owner. A close
    # separates all following preparation/activation records from its lifetime.
    if (event == "sophia_live_native_owner" && status == "closed") {
        if (f["schema"] != 1 || !positive(f["epoch"])) fail("invalid native owner epoch")
        if (f["settled"] != "true" || f["settlement_failures"] + 0 != 0)
            fail("unsettled native owner")
        owner_epoch++
    }
    if (event == "sophia_session_app" && status == "started") {
        if (f["id"] == "terminal" && f["source"] == "startup") terminals++
        if ((f["id"] == "browser" || f["id"] == "firefox") && f["source"] == "action") {
            action_launch()
        }
    }
    if (event == "sophia_session_app" && status == "surface_observed" && f["source"] == "action") {
        if (!launch || !positive(f["surface"])) fail("surface without its action launch")
        if (launch_transaction != "" && launch_transaction != "transaction:" f["transaction"])
            fail("surface transaction does not match its action launch")
        launch_echo = 0
        surfaces++
        browser = f["surface"]
        observed = NR
    }
    if (event == "sophia_firefox_rendering" && status == "page_ready") {
        if (!observed || f["title_bytes"] != 249) fail("invalid page readiness")
        ready = NR
    }
    if (event == "sophia_live_wm" && status == "restarted" && launch) restarts++
    if (event == "sophia_live_resize_epoch" && f["surface"] == browser) {
        if (status == "recovery_extent_retained") fail("retained fallback extent")
        if (status == "recovery_extent_cleared") {
            clears++
            if (f["reason"] == "cpu_admission_committed") cpu_clear = NR
            else if (f["reason"] == "admission_present_retired") present_clears++
            else fail("unsupported admission recovery reason")
        }
        if (status == "visual_armed" && f["source"] == "standing_target_recovery") {
            successors++
            successor = f["transaction"]
            successor_size = f["width"] "x" f["height"]
        }
        if (status == "visual_committed" && successor && f["transaction"] == successor &&
            successor_size == f["width"] "x" f["height"]) successor_retired = NR
    }
    if (event == "sophia_live_visual_admission" && f["schema"] == 1 && status == "committed" &&
        f["surface"] == browser && cpu_clear && !complete &&
        positive(f["transaction"]) && f["source"] == "cpu_backing_snapshot") cpu_admitted = NR
    if (event == "sophia_live_head_content_geometry" && status == "selected" &&
        ready && !complete && f["surface"] == browser) {
        split(f["target"], geometry, "[_x]")
        # Full, unit-scale content only. A clipped strip or tiny startup
        # placeholder is not the browser's configured rendering proof.
        if (geometry[1] >= 64 && geometry[2] >= 64 && f["size"] == geometry[1] "x" geometry[2] &&
            f["clip"] == f["target"] && (f["source"] == "cpu" || f["source"] == "dmabuf")) {
            key = scene_key()
            targets[key] = f["target"]
            areas[key] = geometry[1] * geometry[2]
            selected[key] = NR
        }
    }
    if (event == "sophia_live_head_composition_queue" && status == "queued" && ready && !complete) {
        key = scene_key()
        frame = frame_key()
        frame_scene[frame] = key
        queued[frame] = NR
    }
    if (event == "sophia_native_composition_region_frame" && f["schema"] == 1 && status == "read" && !complete) {
        key = scene_key()
        if (selected[key] > ready && targets[key] == f["target"] && areas[key] == f["region_pixels"] &&
            f["nonzero_rgb_pixels"] > areas[key]/4 && f["nonzero_rgb_pixels"] <= areas[key] &&
            positive(f["checksum"]) && (f["source_stage"] == "cpu" || f["source_stage"] == "dmabuf" || f["source_stage"] == "renderer_image")) {
            pixels[key] = NR
            checksums[key] = "checksum:" f["checksum"]
        }
    }
    if (event == "sophia_live_native_head_page_flip" && f["schema"] == 2 && status == "retired" && !complete) {
        frame = frame_key()
        key = frame_scene[frame]
        if (queued[frame] > ready && pixels[key] > queued[frame] && positive(f["submission"])) {
            retired_frames[frame] = 1
            retired_checksums[checksums[key]] = 1
        }
    }
    if (event == "sophia_live_session_present" && status == "retired" && f["surface"] == browser && ready && !complete) {
        if ((f["schema"] == 2 && f["unit_scale"] == "true") ||
            (f["schema"] == 4 && f["kind"] == "software" && positive(f["native_submission"]))) presents++
    }
    if (event == "sophia_firefox_rendering" && status == "complete") {
        if (!ready || f["page_ready"] != "true" || f["recovery_extents"] !~ /^0$/) fail("incomplete page lifecycle")
        complete = NR
    }
    if (event == "sophia_live_session_health" && complete && status == "clean" &&
        f["protocol_errors"] ~ /^0$/ && f["pending_wm"] ~ /^0$/ && f["pending_actions"] ~ /^0$/ &&
        f["pending_input"] ~ /^0$/ && f["wm_degraded"] == "false") health = NR
    if (event == "sophia_live_layout_health" && complete && status == "clean" &&
        f["recovery_extents"] ~ /^0$/ && f["standing_targets"] ~ /^0$/ && f["constraint_relayout_pending"] == "false") layout = NR
    if (event == "sophia_live_session_cleanup" && health && layout && status == "clean" &&
        f["app_groups"] ~ /^0$/ && f["frontend_workers"] ~ /^0$/ && f["namespace"] == "revoked" && f["xauthority"] == "removed") cleanup = NR
}
END {
    if (failed) exit 1
    if (terminals != 1 || launches != 1 || surfaces != 1) fail("expected one terminal and one browser action surface")
    if (!ready || !complete || !presents) fail("missing readiness, surface retirement, or completion")
    if (length(retired_frames) < 2 || length(retired_checksums) < 2)
        fail("need two changing nonblack browser frames with exact native retirement")
    # CPU backing can commit without a standing-target successor. Present-based
    # recovery still owes one, and every observed successor must retire.
    if (restarts > 1 || clears > 1 || successors > clears ||
        (present_clears && successors != 1) || (successors && !successor_retired) ||
        (cpu_clear && cpu_admitted <= cpu_clear))
        fail("repeated or unfinished layout recovery")
    if (!health || !layout || !cleanup) fail("session, layout, or application cleanup did not drain")
}
