//! Passive replay and reduction of one raw desktop-comparison attempt.

use super::{
    FIREFOX_VERSION, KITTY_VERSION, NIRI_VERSION, TOPOLOGY, XLIBRE_COMMIT, schedule, source_commit,
};
use sha2::Digest as _;
use std::fs;
use std::path::Path;

const ATTEMPT_PREFIX: &str = "desktop_comparison_attempt schema=1 status=measured ";
const RESOURCE_PREFIX: &str = "desktop_comparison_resource schema=1 ";
const KERNEL_FRAME_PREFIX: &str = "desktop_comparison_kernel_frame schema=1 ";
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
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Distribution {
    samples: usize,
    mean: u64,
    p50: u64,
    p95: u64,
    p99: u64,
    max: u64,
}

pub fn replay_attempt(run: &Path, attempt: &Path) -> Result<CaptureReplay, String> {
    let candidate = source_commit(run)?;
    let attempt_source = read_one(attempt.join("attempt.kdl"), ATTEMPT_PREFIX)?;
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
        ("crashes", "0"),
        ("sample_loss", "0"),
    ] {
        if required(&attempt_fields, name)? != expected {
            return Err(format!(
                "capture attempt {name} does not match the prepared contract"
            ));
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

    let sample_record = format!(
        "desktop_comparison_sample schema=2 status=complete order={} stack={} workload={} repetition={} backend=native stack_version={} topology={} kitty={} firefox={} duration_msec={} resource_samples={} processes={} pss_peak_kib={} rss_peak_kib={} anonymous_peak_kib={} private_dirty_peak_kib={} cpu_msec={} minor_faults={} major_faults={} threads_peak={} fds_peak={} launch_msec={} settle_msec={} resize_msec={} resize_samples={} resize_p50_usec={} resize_p95_usec={} resize_p99_usec={} resize_max_usec={} frame_source=kernel_drm frame_samples={} frame_mean_usec={} frame_p50_usec={} frame_p95_usec={} frame_p99_usec={} frame_max_usec={} native_timing={} native_source={} native_samples={} crashes=0 sample_loss=0 teardown=clean",
        scheduled.order,
        scheduled.stack,
        scheduled.workload,
        scheduled.repetition,
        expected_version,
        TOPOLOGY,
        KITTY_VERSION,
        FIREFOX_VERSION,
        duration_msec,
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
        micros_to_millis(launch_usec),
        micros_to_millis(settle_usec),
        micros_to_millis(resize_p95_usec),
        resize_samples,
        resize_p50_usec,
        resize_p95_usec,
        resize_p99_usec,
        resize_max_usec,
        frames.samples,
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

fn replay_resources(attempt: &Path, duration_msec: u64) -> Result<(usize, ResourcePeak), String> {
    let records = read_records(attempt.join("resources.log"), RESOURCE_PREFIX)?;
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
    let records = read_records(attempt.join("kernel-frames.log"), KERNEL_FRAME_PREFIX)?;
    if records.len() < 3 {
        return Err("kernel timing requires at least three completion timestamps".to_owned());
    }
    let mut previous_usec = 0;
    let mut crtc = None;
    let mut intervals = Vec::with_capacity(records.len().saturating_sub(1));
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
        let ust_usec = number(record, "ust_usec")?;
        if previous_usec != 0 {
            if ust_usec <= previous_usec {
                return Err("kernel completion clock is not strictly monotonic".to_owned());
            }
            intervals.push(ust_usec - previous_usec);
        }
        previous_usec = ust_usec;
    }
    distribution(intervals)
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

pub(crate) const RAW_ATTEMPT_FILES: [&str; 5] = [
    "attempt.kdl",
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
