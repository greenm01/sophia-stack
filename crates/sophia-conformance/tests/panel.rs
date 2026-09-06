use sophia_conformance::panel::verify;

fn sample(seq: u64, surface: &str, geometry: &str, generation: u64) -> String {
    format!(
        "sophia_live_cpu_surface schema=1 seq={seq} surface={surface} {geometry} buffer_generation={generation} visual_detail=true truncated=false\n"
    )
}
fn evidence() -> String {
    let mut log = String::from(
        "sophia_live_session schema=7 native_presentation=disabled physical_input=disabled wm_policy=external\nLoading backend software\n",
    );
    for (seq, generation) in [(1, 2), (2, 3), (3, 4), (5, 5)] {
        log.push_str(&sample(
            seq,
            "1:1",
            "x=0 y=0 width=1280 height=32",
            generation,
        ));
    }
    log.push_str(&sample(2, "2:1", "x=1032 y=32 width=240 height=112", 1));
    log.push_str(&sample(3, "2:1", "x=1032 y=32 width=240 height=112", 2));
    log.push_str(&sample(5, "3:1", "x=321 y=33 width=638 height=686", 4));
    for (y, height, count) in [(32, 688, 1), (14, 706, 0), (32, 688, 1), (14, 706, 0)] {
        log.push_str(&format!("sophia_live_work_area schema=1 output=1 x=0 y={y} width=1280 height={height} app_reservations={count} shell_reservations=0\n"));
    }
    log.push_str("sophia_live_session_protocol_error_tally schema=2 status=clean count=0\nsophia_live_session_cleanup schema=1 status=clean namespace=revoked app_groups=0 frontend_workers=0\n");
    log
}
#[test]
fn accepts_cpu_lifecycle_without_claiming_physical_acceptance() {
    let verdict = verify(&evidence()).unwrap();
    assert!(verdict.contains("gpu_presentation=unproven"));
    assert!(verdict.contains("physical_input=unproven"));
}
#[test]
fn rejects_missing_evidence_and_false_success() {
    assert!(verify("").is_err());
    let complete = evidence();
    for marker in [
        "cpu_surface",
        "work_area",
        "cleanup",
        "protocol_error_tally",
        "Loading backend",
        "width=638",
    ] {
        let incomplete = complete
            .lines()
            .filter(|line| !line.contains(marker))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(verify(&incomplete).is_err(), "missing {marker}");
    }
    for (from, to) in [
        ("visual_detail=true", "visual_detail=false"),
        ("truncated=false", "truncated=true"),
        ("buffer_generation=2", "buffer_generation=1"),
        ("x=1032", "x=1000"),
        ("status=clean count=0", "status=degraded count=1"),
    ] {
        assert!(
            verify(&complete.replace(from, to)).is_err(),
            "changed {from}"
        );
    }
    assert!(verify(&(complete + "Error: session failed\n")).is_err());
}

#[test]
fn decorated_records_and_blank_retained_popup_are_handled() {
    let log = evidence();
    let decorated = log
        .lines()
        .map(|line| format!("2026-09-05T00:00:00Z INFO session: {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(verify(&decorated).is_ok());
    let still_present = sample(6, "2:1", "x=1032 y=32 width=240 height=112", 3)
        .replace("visual_detail=true", "visual_detail=false");
    assert!(verify(&(log + &still_present)).is_err());
}
