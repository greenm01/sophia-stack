//! Passive replay and reduction of one raw desktop-comparison attempt.

use super::{
    FIREFOX_VERSION, KITTY_VERSION, NIRI_VERSION, TOPOLOGY, XLIBRE_COMMIT, schedule, source_commit,
};
use sha2::Digest as _;
use std::fs;
use std::path::Path;

const ATTEMPT_PREFIXES: [&str; 2] = [
    "desktop_comparison_attempt schema=3 status=measured ",
    "desktop_comparison_attempt schema=2 status=measured ",
];
const VISIBILITY_PREFIX: &str = "desktop_comparison_visibility schema=1 ";
const RESOURCE_PREFIXES: [&str; 2] = [
    "desktop_comparison_resource schema=2 ",
    "desktop_comparison_resource schema=1 ",
];
const KERNEL_FRAME_PREFIXES: [&str; 2] = [
    "desktop_comparison_kernel_frame schema=2 ",
    "desktop_comparison_kernel_frame schema=1 ",
];
const WORKLOAD_PREFIX: &str = "desktop_comparison_workload schema=1 status=complete ";
const NATIVE_PREFIX: &str = "desktop_comparison_native_timing schema=1 ";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureReplay {
    pub order: usize,
    pub stack: String,
    pub workload: String,
    pub repetition: u8,
    pub sample_record: String,
}

#[derive(Clone, Copy, Debug)]
struct ResourcePeak {
    processes: u64,
    pss_kib: u64,
    rss_kib: u64,
    anonymous_kib: u64,
    private_dirty_kib: u64,
    cpu_msec: u64,
    minor_faults: u64,
    major_faults: u64,
    threads: u64,
    fds: u64,
    stack_processes: u64,
    stack_pss_kib: u64,
    stack_rss_kib: u64,
    stack_cpu_msec: u64,
    stack_threads: u64,
    stack_fds: u64,
    workload_processes: u64,
    workload_pss_kib: u64,
    workload_rss_kib: u64,
    workload_cpu_msec: u64,
    workload_threads: u64,
    workload_fds: u64,
}

impl ResourcePeak {
    const fn empty() -> Self {
        Self {
            processes: 0,
            pss_kib: 0,
            rss_kib: 0,
            anonymous_kib: 0,
            private_dirty_kib: 0,
            cpu_msec: 0,
            minor_faults: 0,
            major_faults: 0,
            threads: 0,
            fds: 0,
            stack_processes: 0,
            stack_pss_kib: 0,
            stack_rss_kib: 0,
            stack_cpu_msec: 0,
            stack_threads: 0,
            stack_fds: 0,
            workload_processes: 0,
            workload_pss_kib: 0,
            workload_rss_kib: 0,
            workload_cpu_msec: 0,
            workload_threads: 0,
            workload_fds: 0,
        }
    }

    fn include(
        &mut self,
        record: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        self.processes = self.processes.max(number(record, "processes")?);
        self.pss_kib = self.pss_kib.max(number(record, "pss_kib")?);
        self.rss_kib = self.rss_kib.max(number(record, "rss_kib")?);
        self.anonymous_kib = self.anonymous_kib.max(number(record, "anonymous_kib")?);
        self.private_dirty_kib = self
            .private_dirty_kib
            .max(number(record, "private_dirty_kib")?);
        self.cpu_msec = self.cpu_msec.max(number(record, "cpu_msec")?);
        self.minor_faults = self.minor_faults.max(number(record, "minor_faults")?);
        self.major_faults = self.major_faults.max(number(record, "major_faults")?);
        self.threads = self.threads.max(number(record, "threads")?);
        self.fds = self.fds.max(number(record, "fds")?);
        self.stack_processes = self
            .stack_processes
            .max(optional_number(record, "stack_processes")?);
        self.stack_pss_kib = self
            .stack_pss_kib
            .max(optional_number(record, "stack_pss_kib")?);
        self.stack_rss_kib = self
            .stack_rss_kib
            .max(optional_number(record, "stack_rss_kib")?);
        self.stack_cpu_msec = self
            .stack_cpu_msec
            .max(optional_number(record, "stack_cpu_msec")?);
        self.stack_threads = self
            .stack_threads
            .max(optional_number(record, "stack_threads")?);
        self.stack_fds = self.stack_fds.max(optional_number(record, "stack_fds")?);
        self.workload_processes = self
            .workload_processes
            .max(optional_number(record, "workload_processes")?);
        self.workload_pss_kib = self
            .workload_pss_kib
            .max(optional_number(record, "workload_pss_kib")?);
        self.workload_rss_kib = self
            .workload_rss_kib
            .max(optional_number(record, "workload_rss_kib")?);
        self.workload_cpu_msec = self
            .workload_cpu_msec
            .max(optional_number(record, "workload_cpu_msec")?);
        self.workload_threads = self
            .workload_threads
            .max(optional_number(record, "workload_threads")?);
        self.workload_fds = self
            .workload_fds
            .max(optional_number(record, "workload_fds")?);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Distribution {
    samples: usize,
    deliveries: u64,
    duplicates: u64,
    mean: u64,
    p50: u64,
    p95: u64,
    p99: u64,
    max: u64,
}

pub fn replay_attempt(run: &Path, attempt: &Path) -> Result<CaptureReplay, String> {
    let candidate = source_commit(run)?;
    let attempt_source = read_one_any(attempt.join("attempt.kdl"), &ATTEMPT_PREFIXES)?;
    let staged_schema =
        attempt_source.starts_with("desktop_comparison_attempt schema=3 status=measured ");
    let attempt_fields = owned_fields(&attempt_source)?;
    let order = integer::<usize>(&attempt_fields, "order")?;
    let repetition = integer::<u8>(&attempt_fields, "repetition")?;
    let stack = required(&attempt_fields, "stack")?;
    let workload = required(&attempt_fields, "workload")?;
    let scheduled = schedule()
        .into_iter()
        .find(|item| {
            item.order == order
                && item.stack == stack
                && item.workload == workload
                && item.repetition == repetition
        })
        .ok_or_else(|| "capture attempt does not name a scheduled row".to_owned())?;

    let expected_version = match stack {
        "sophia" => candidate.as_str(),
        "xlibre-xmonad" => XLIBRE_COMMIT,
        "niri" => NIRI_VERSION,
        _ => return Err(format!("unknown comparison stack {stack:?}")),
    };
    for (name, expected) in [
        ("backend", "native"),
        ("stack_version", expected_version),
        ("topology", TOPOLOGY),
        ("kitty", KITTY_VERSION),
        ("firefox", FIREFOX_VERSION),
        ("teardown", "clean"),
        ("controller_outside_supervisor", "true"),
        ("crashes", "0"),
        ("sample_loss", "0"),
    ] {
        if required(&attempt_fields, name)? != expected {
            return Err(format!(
                "capture attempt {name} does not match the prepared contract"
            ));
        }
    }
    if staged_schema {
        if required(&attempt_fields, "supervisor_exited")? != "true" {
            return Err("staged capture was not finalized after supervisor exit".to_owned());
        }
        let expected_qualification = if stack == "sophia" && order == 1 {
            "passed"
        } else {
            "not_required"
        };
        if required(&attempt_fields, "cursor_qualification")? != expected_qualification {
            return Err("capture cursor qualification does not match its schedule row".to_owned());
        }
        let targets = number(&attempt_fields, "cursor_targets")?;
        let motions = number(&attempt_fields, "cursor_motion_events")?;
        if expected_qualification == "passed" && (targets != 4 || motions == 0) {
            return Err("first Sophia row lacks complete visual cursor qualification".to_owned());
        }
        if expected_qualification == "not_required" && (targets != 0 || motions != 0) {
            return Err("non-qualification row claims cursor target evidence".to_owned());
        }
    }
    let duration_msec = number(&attempt_fields, "duration_msec")?;
    let minimum_duration = if workload == "soak-2h" {
        7_200_000
    } else {
        60_000
    };
    if duration_msec < minimum_duration {
        return Err(format!(
            "capture duration is below the {minimum_duration}ms workload window"
        ));
    }

    let (resource_samples, resources) = replay_resources(attempt, duration_msec)?;
    let visibility_samples = replay_visibility(attempt, resource_samples)?;
    if number(&attempt_fields, "visibility_samples")?
        != u64::try_from(visibility_samples).unwrap_or(u64::MAX)
    {
        return Err(
            "capture attempt visibility_samples does not match visibility evidence".to_owned(),
        );
    }
    let frames = replay_kernel_frames(attempt)?;
    let workload_source = read_one(attempt.join("workload.log"), WORKLOAD_PREFIX)?;
    let workload_fields = owned_fields(&workload_source)?;
    let launch_usec = number(&workload_fields, "launch_usec")?;
    let settle_usec = number(&workload_fields, "settle_usec")?;
    let resize_samples = number(&workload_fields, "resize_samples")?;
    let resize_p50_usec = number(&workload_fields, "resize_p50_usec")?;
    let resize_p95_usec = number(&workload_fields, "resize_p95_usec")?;
    let resize_p99_usec = number(&workload_fields, "resize_p99_usec")?;
    let resize_max_usec = number(&workload_fields, "resize_max_usec")?;
    if workload == "resize"
        && (resize_samples != 120
            || resize_p50_usec == 0
            || resize_p95_usec == 0
            || resize_p99_usec == 0
            || resize_max_usec == 0)
    {
        return Err("resize capture requires 120 positive latency samples".to_owned());
    }

    let native_source = read_one(attempt.join("native.log"), NATIVE_PREFIX)?;
    let native_fields = owned_fields(&native_source)?;
    let native_availability = required(&native_fields, "availability")?;
    if !matches!(native_availability, "available" | "not_exposed") {
        return Err("native timing availability must be available or not_exposed".to_owned());
    }
    let native_samples = number(&native_fields, "samples")?;
    if native_availability == "available" && native_samples == 0 {
        return Err("available native timing requires positive samples".to_owned());
    }
    if native_availability == "not_exposed" && native_samples != 0 {
        return Err("unexposed native timing cannot claim samples".to_owned());
    }
    let native_source_name = required(&native_fields, "source")?;
    let sample_schema = if resources.stack_processes > 0 && resources.workload_processes > 0 {
        4
    } else {
        3
    };
    if staged_schema && sample_schema != 4 {
        return Err("staged capture lacks split stack/workload resources".to_owned());
    }

    let sample_record = format!(
        "desktop_comparison_sample schema={sample_schema} status=complete order={} stack={} workload={} repetition={} backend=native stack_version={} topology={} kitty={} firefox={} duration_msec={} controller_outside_supervisor=true visibility_samples={} visible_dp1=true focused_owned=true foreign_toplevels=0 resource_samples={} processes={} pss_peak_kib={} rss_peak_kib={} anonymous_peak_kib={} private_dirty_peak_kib={} cpu_msec={} minor_faults={} major_faults={} threads_peak={} fds_peak={} stack_processes={} stack_pss_peak_kib={} stack_rss_peak_kib={} stack_cpu_msec={} stack_threads_peak={} stack_fds_peak={} workload_processes={} workload_pss_peak_kib={} workload_rss_peak_kib={} workload_cpu_msec={} workload_threads_peak={} workload_fds_peak={} launch_msec={} settle_msec={} resize_msec={} resize_samples={} resize_p50_usec={} resize_p95_usec={} resize_p99_usec={} resize_max_usec={} frame_source=kernel_drm frame_samples={} frame_deliveries={} frame_duplicates={} frame_mean_usec={} frame_p50_usec={} frame_p95_usec={} frame_p99_usec={} frame_max_usec={} native_timing={} native_source={} native_samples={} crashes=0 sample_loss=0 teardown=clean",
        scheduled.order,
        scheduled.stack,
        scheduled.workload,
        scheduled.repetition,
        expected_version,
        TOPOLOGY,
        KITTY_VERSION,
        FIREFOX_VERSION,
        duration_msec,
        visibility_samples,
        resource_samples,
        resources.processes,
        resources.pss_kib,
        resources.rss_kib,
        resources.anonymous_kib,
        resources.private_dirty_kib,
        resources.cpu_msec,
        resources.minor_faults,
        resources.major_faults,
        resources.threads,
        resources.fds,
        resources.stack_processes,
        resources.stack_pss_kib,
        resources.stack_rss_kib,
        resources.stack_cpu_msec,
        resources.stack_threads,
        resources.stack_fds,
        resources.workload_processes,
        resources.workload_pss_kib,
        resources.workload_rss_kib,
        resources.workload_cpu_msec,
        resources.workload_threads,
        resources.workload_fds,
        micros_to_millis(launch_usec),
        micros_to_millis(settle_usec),
        micros_to_millis(resize_p95_usec),
        resize_samples,
        resize_p50_usec,
        resize_p95_usec,
        resize_p99_usec,
        resize_max_usec,
        frames.samples,
        frames.deliveries,
        frames.duplicates,
        frames.mean,
        frames.p50,
        frames.p95,
        frames.p99,
        frames.max,
        native_availability,
        native_source_name,
        native_samples,
    );
    Ok(CaptureReplay {
        order,
        stack: stack.to_owned(),
        workload: workload.to_owned(),
        repetition,
        sample_record,
    })
}

fn replay_visibility(attempt: &Path, resource_samples: usize) -> Result<usize, String> {
    let records = read_records(attempt.join("visibility.log"), VISIBILITY_PREFIX)?;
    let expected_records = resource_samples.saturating_add(2);
    if records.len() != expected_records {
        return Err(format!(
            "visibility series has {} records; expected {expected_records}",
            records.len()
        ));
    }

    let baseline = &records[0];
    if required(baseline, "phase")? != "baseline"
        || number(baseline, "seq")? != 0
        || number(baseline, "monotonic_usec")? != 0
        || number(baseline, "owned_toplevels")? != 0
        || number(baseline, "visible_dp1")? != 0
        || number(baseline, "foreign_toplevels")? != 0
        || required(baseline, "focused_visible_dp1")? != "false"
    {
        return Err("visibility baseline is not an empty application tree".to_owned());
    }

    let settled = &records[1];
    if required(settled, "phase")? != "settled"
        || number(settled, "seq")? != 0
        || number(settled, "monotonic_usec")? != 0
    {
        return Err("visibility settled record is malformed".to_owned());
    }
    require_visible_observation(settled)?;

    let mut previous_usec = 0;
    for (index, record) in records.iter().skip(2).enumerate() {
        let expected = u64::try_from(index + 1).unwrap_or(u64::MAX);
        if required(record, "phase")? != "sample" || number(record, "seq")? != expected {
            return Err("visibility sample sequence is not contiguous from one".to_owned());
        }
        let monotonic_usec = number(record, "monotonic_usec")?;
        if monotonic_usec <= previous_usec {
            return Err("visibility sample clock is not strictly monotonic".to_owned());
        }
        let expected_usec = expected.saturating_mul(1_000_000);
        if monotonic_usec.abs_diff(expected_usec) > 500_000 {
            return Err(format!(
                "visibility sample cadence drifted outside 500ms at sequence {expected}"
            ));
        }
        require_visible_observation(record)?;
        previous_usec = monotonic_usec;
    }
    Ok(resource_samples)
}

fn require_visible_observation(
    record: &std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    if number(record, "owned_toplevels")? == 0 {
        return Err("visibility evidence has no workload-owned toplevel".to_owned());
    }
    if number(record, "visible_dp1")? == 0 {
        return Err("visibility evidence has no workload-owned DP-1 toplevel".to_owned());
    }
    if number(record, "foreign_toplevels")? != 0 {
        return Err("visibility evidence contains a foreign application toplevel".to_owned());
    }
    if required(record, "focused_visible_dp1")? != "true" {
        return Err("visibility evidence lacks focused workload ownership on DP-1".to_owned());
    }
    Ok(())
}

fn replay_resources(attempt: &Path, duration_msec: u64) -> Result<(usize, ResourcePeak), String> {
    let records = read_records_any(attempt.join("resources.log"), &RESOURCE_PREFIXES)?;
    let minimum = usize::try_from(duration_msec / 1_000)
        .unwrap_or(usize::MAX)
        .saturating_sub(1);
    if records.len() < minimum {
        return Err(format!(
            "resource series is truncated: found {} samples, need at least {minimum}",
            records.len()
        ));
    }
    let mut previous_usec = 0;
    let mut peak = ResourcePeak::empty();
    for (index, record) in records.iter().enumerate() {
        let expected = u64::try_from(index + 1).unwrap_or(u64::MAX);
        if number(record, "seq")? != expected {
            return Err("resource sample sequence is not contiguous from one".to_owned());
        }
        let monotonic_usec = number(record, "monotonic_usec")?;
        if monotonic_usec <= previous_usec {
            return Err("resource sample clock is not strictly monotonic".to_owned());
        }
        let expected_usec = expected.saturating_mul(1_000_000);
        if monotonic_usec.abs_diff(expected_usec) > 500_000 {
            return Err(format!(
                "resource sample cadence drifted outside 500ms at sequence {expected}"
            ));
        }
        previous_usec = monotonic_usec;
        peak.include(record)?;
    }
    if peak.processes == 0
        || peak.pss_kib == 0
        || peak.rss_kib == 0
        || peak.threads == 0
        || peak.fds == 0
    {
        return Err("resource series lacks a positive process population".to_owned());
    }
    Ok((records.len(), peak))
}

fn replay_kernel_frames(attempt: &Path) -> Result<Distribution, String> {
    let records = read_records_any(attempt.join("kernel-frames.log"), &KERNEL_FRAME_PREFIXES)?;
    if records.len() < 3 {
        return Err("kernel timing requires at least three completion timestamps".to_owned());
    }
    let mut previous_usec = 0;
    let mut crtc = None;
    let mut intervals = Vec::with_capacity(records.len().saturating_sub(1));
    let mut deliveries = 0u64;
    let mut kernel_sequences = std::collections::BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        let expected = u64::try_from(index + 1).unwrap_or(u64::MAX);
        if number(record, "seq")? != expected {
            return Err("kernel frame sequence is not contiguous from one".to_owned());
        }
        let record_crtc = number(record, "crtc")?;
        if crtc
            .replace(record_crtc)
            .is_some_and(|value| value != record_crtc)
        {
            return Err("kernel timing mixes CRTC identities".to_owned());
        }
        let kernel_sequence = optional_number(record, "kernel_sequence")?;
        if kernel_sequence != 0 && !kernel_sequences.insert(kernel_sequence) {
            return Err("normalized kernel frame repeats a kernel sequence".to_owned());
        }
        let record_deliveries = record.get("deliveries").map_or(Ok(1), |value| {
            value
                .parse::<u64>()
                .map_err(|_| "capture deliveries is not an integer".to_owned())
        })?;
        if record_deliveries == 0 {
            return Err("normalized kernel frame has no delivered event".to_owned());
        }
        deliveries = deliveries.saturating_add(record_deliveries);
        let ust_usec = number(record, "ust_usec")?;
        if previous_usec != 0 {
            if ust_usec <= previous_usec {
                return Err("kernel completion clock is not strictly monotonic".to_owned());
            }
            intervals.push(ust_usec - previous_usec);
        }
        previous_usec = ust_usec;
    }
    let mut result = distribution(intervals)?;
    result.deliveries = deliveries;
    result.duplicates = deliveries.saturating_sub(u64::try_from(records.len()).unwrap_or(u64::MAX));
    Ok(result)
}

fn distribution(mut values: Vec<u64>) -> Result<Distribution, String> {
    if values.is_empty() {
        return Err("timing distribution is empty".to_owned());
    }
    values.sort_unstable();
    let count = u64::try_from(values.len()).unwrap_or(u64::MAX);
    let sum = values.iter().copied().fold(0u64, u64::saturating_add);
    Ok(Distribution {
        samples: values.len(),
        deliveries: u64::try_from(values.len().saturating_add(1)).unwrap_or(u64::MAX),
        duplicates: 0,
        mean: sum / count,
        p50: percentile(&values, 50),
        p95: percentile(&values, 95),
        p99: percentile(&values, 99),
        max: *values.last().expect("nonempty distribution"),
    })
}

fn percentile(values: &[u64], percentile: usize) -> u64 {
    let rank = values.len().saturating_mul(percentile).saturating_add(99) / 100;
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

fn read_one(path: impl AsRef<Path>, prefix: &str) -> Result<String, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let mut records = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if records.len() != 1 || !records[0].starts_with(prefix) {
        return Err(format!(
            "{} requires exactly one {prefix:?} record",
            path.display()
        ));
    }
    Ok(records.remove(0).to_owned())
}

fn read_one_any(path: impl AsRef<Path>, prefixes: &[&str]) -> Result<String, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let records = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if records.len() != 1 || !prefixes.iter().any(|prefix| records[0].starts_with(prefix)) {
        return Err(format!("{} contains an unexpected record", path.display()));
    }
    Ok(records[0].to_owned())
}

fn read_records(
    path: impl AsRef<Path>,
    prefix: &str,
) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if source
        .lines()
        .any(|line| !line.is_empty() && !line.starts_with(prefix))
    {
        return Err(format!("{} contains an unexpected record", path.display()));
    }
    let records = source
        .lines()
        .filter(|line| line.starts_with(prefix))
        .map(owned_fields)
        .collect::<Result<Vec<_>, _>>()?;
    if records.is_empty() {
        return Err(format!("{} contains no records", path.display()));
    }
    Ok(records)
}

fn read_records_any(
    path: impl AsRef<Path>,
    prefixes: &[&str],
) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let selected = source
        .lines()
        .find(|line| !line.is_empty())
        .and_then(|line| prefixes.iter().find(|prefix| line.starts_with(**prefix)))
        .copied()
        .ok_or_else(|| format!("{} contains no records", path.display()))?;
    if source
        .lines()
        .any(|line| !line.is_empty() && !line.starts_with(selected))
    {
        return Err(format!("{} contains an unexpected record", path.display()));
    }
    let records = source
        .lines()
        .filter(|line| line.starts_with(selected))
        .map(owned_fields)
        .collect::<Result<Vec<_>, _>>()?;
    if records.is_empty() {
        return Err(format!("{} contains no records", path.display()));
    }
    Ok(records)
}

fn owned_fields(line: &str) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut result = std::collections::BTreeMap::new();
    for token in line.split_ascii_whitespace() {
        let Some((name, value)) = token.split_once('=') else {
            continue;
        };
        if result.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(format!("desktop comparison record repeats field {name}"));
        }
    }
    Ok(result)
}

fn required<'a>(
    record: &'a std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    record
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("capture record lacks {name}"))
}

fn number(record: &std::collections::BTreeMap<String, String>, name: &str) -> Result<u64, String> {
    required(record, name)?
        .parse()
        .map_err(|_| format!("capture {name} is not an integer"))
}

fn optional_number(
    record: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<u64, String> {
    record.get(name).map_or(Ok(0), |value| {
        value
            .parse()
            .map_err(|_| format!("capture {name} is not an integer"))
    })
}

fn integer<T>(record: &std::collections::BTreeMap<String, String>, name: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    required(record, name)?
        .parse()
        .map_err(|_| format!("capture {name} is not an integer"))
}

fn micros_to_millis(value: u64) -> u64 {
    value.saturating_add(999) / 1_000
}

pub(crate) const RAW_ATTEMPT_FILES: [&str; 6] = [
    "attempt.kdl",
    "visibility.log",
    "resources.log",
    "kernel-frames.log",
    "workload.log",
    "native.log",
];

pub(crate) fn archive_attempt(
    run: &Path,
    source: &Path,
    destination: &Path,
) -> Result<CaptureReplay, String> {
    if destination.exists() {
        return verify_archived_attempt(run, destination);
    }
    let partial = destination.with_extension("partial");
    if partial.exists() {
        return Err(format!(
            "partial comparison attempt needs diagnosis: {}",
            partial.display()
        ));
    }
    fs::create_dir_all(
        partial
            .parent()
            .ok_or("comparison attempt destination has no parent")?,
    )
    .and_then(|()| fs::create_dir(&partial))
    .map_err(|error| format!("could not create comparison attempt: {error}"))?;
    for name in RAW_ATTEMPT_FILES {
        fs::copy(source.join(name), partial.join(name))
            .map_err(|error| format!("could not copy raw attempt {name}: {error}"))?;
    }
    let replay = replay_attempt(run, &partial)?;
    write_new(
        &partial.join("result.kdl"),
        format!("{}\n", replay.sample_record).as_bytes(),
    )?;
    let mut checksums = String::new();
    for name in RAW_ATTEMPT_FILES
        .into_iter()
        .chain(std::iter::once("result.kdl"))
    {
        checksums.push_str(&format!("{}  {name}\n", digest(&partial.join(name))?));
    }
    write_new(&partial.join("checksums.sha256"), checksums.as_bytes())?;
    verify_archived_attempt(run, &partial)?;
    fs::rename(&partial, destination)
        .map_err(|error| format!("could not seal comparison attempt: {error}"))?;
    Ok(replay)
}

pub(crate) fn verify_archived_attempt(run: &Path, attempt: &Path) -> Result<CaptureReplay, String> {
    let expected_entries = RAW_ATTEMPT_FILES
        .into_iter()
        .chain(["result.kdl", "checksums.sha256"])
        .collect::<std::collections::BTreeSet<_>>();
    let mut observed_entries = std::collections::BTreeSet::new();
    for entry in fs::read_dir(attempt)
        .map_err(|error| format!("could not enumerate archived attempt: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("could not inspect archived attempt entry: {error}"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "archived attempt contains a non-UTF-8 entry")?;
        if !entry
            .file_type()
            .map_err(|error| format!("could not inspect archived attempt type: {error}"))?
            .is_file()
            || !expected_entries.contains(name.as_str())
            || !observed_entries.insert(name)
        {
            return Err("archived attempt contains an unowned or non-file entry".to_owned());
        }
    }
    if observed_entries.len() != expected_entries.len() {
        return Err("archived attempt artifact set is incomplete".to_owned());
    }
    let checksums = fs::read_to_string(attempt.join("checksums.sha256"))
        .map_err(|error| format!("attempt checksums are missing: {error}"))?;
    let expected = RAW_ATTEMPT_FILES
        .into_iter()
        .chain(std::iter::once("result.kdl"))
        .collect::<std::collections::BTreeSet<_>>();
    let mut observed = std::collections::BTreeSet::new();
    for line in checksums.lines() {
        let (digest, name) = line
            .split_once("  ")
            .ok_or("attempt checksum line is malformed")?;
        if !expected.contains(name) || !observed.insert(name) {
            return Err("attempt checksum set is duplicate or unexpected".to_owned());
        }
        if digest != self::digest(&attempt.join(name))? {
            return Err(format!("attempt checksum mismatch: {name}"));
        }
    }
    if observed != expected {
        return Err("attempt checksum set is incomplete".to_owned());
    }
    let replay = replay_attempt(run, attempt)?;
    let result = fs::read_to_string(attempt.join("result.kdl"))
        .map_err(|error| format!("attempt result is missing: {error}"))?;
    if result != format!("{}\n", replay.sample_record) {
        return Err("attempt result does not match raw replay".to_owned());
    }
    Ok(replay)
}

fn digest(path: &Path) -> Result<String, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok(format!("{:x}", sha2::Sha256::digest(bytes)))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;

    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}
