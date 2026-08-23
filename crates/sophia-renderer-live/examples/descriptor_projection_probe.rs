use std::error::Error;
use std::time::Instant;

use sophia_engine::{
    ChromeDescriptorTable, CompositorDisplayList, DescriptorOverlayCandidate,
    DescriptorOverlayEntry, HeadRenderTarget, RenderHeadId, ToplevelActionCapabilityRef,
    build_output_head_plans, descriptor_overlay_projection,
    output_scene_snapshot_from_committed_in_view,
};
use sophia_protocol::{
    AttentionState, ChromeDescriptor, DisplayLabel, OutputHeadMapping, OutputId, OutputTransform,
    Rect, Size, SurfaceId, TrustLevel,
};
use sophia_renderer_live::{
    CompositorTextRasterCache, IndicatorStripRasterCache, lower_head_composition_plan_with_caches,
};

const OUTPUT: OutputId = OutputId::from_raw(1);
const ENTRIES: u16 = 16;
const HEADS: usize = 2;
const ITERATIONS: usize = 256;
const FRAME_BUDGET_MICROSECONDS: u128 = 16_667;

fn main() -> Result<(), Box<dyn Error>> {
    let (descriptors, candidate) = reference_input();
    let targets = [target(1, 1_920, 1_080), target(2, 1_280, 720)];
    let viewport = Rect {
        x: 0,
        y: 0,
        width: 1_920,
        height: 1_080,
    };
    let mut indicator_cache = IndicatorStripRasterCache::default();
    let mut text_cache = CompositorTextRasterCache::default();

    run_once(
        &descriptors,
        &candidate,
        viewport,
        &targets,
        &mut indicator_cache,
        &mut text_cache,
    )?;

    let mut elapsed = Vec::with_capacity(ITERATIONS);
    let mut command_count = 0;
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        command_count = run_once(
            &descriptors,
            &candidate,
            viewport,
            &targets,
            &mut indicator_cache,
            &mut text_cache,
        )?;
        elapsed.push(started.elapsed().as_micros());
    }
    elapsed.sort_unstable();
    let p95_index = (ITERATIONS * 95).div_ceil(100).saturating_sub(1);
    let p95 = elapsed[p95_index];
    let status = if p95 <= FRAME_BUDGET_MICROSECONDS {
        "complete"
    } else {
        "budget_exceeded"
    };
    println!(
        "sophia_descriptor_projection_probe schema=1 status={status} entries={ENTRIES} heads={HEADS} iterations={ITERATIONS} commands={command_count} p95_usec={p95} budget_usec={FRAME_BUDGET_MICROSECONDS} text_cache_hits={} text_cache_misses={}",
        text_cache.stats().hits,
        text_cache.stats().misses,
    );
    if p95 > FRAME_BUDGET_MICROSECONDS {
        return Err(format!(
            "descriptor projection p95 {p95} us exceeded {FRAME_BUDGET_MICROSECONDS} us"
        )
        .into());
    }
    Ok(())
}

fn run_once(
    descriptors: &ChromeDescriptorTable,
    candidate: &DescriptorOverlayCandidate,
    viewport: Rect,
    targets: &[HeadRenderTarget; HEADS],
    indicator_cache: &mut IndicatorStripRasterCache,
    text_cache: &mut CompositorTextRasterCache,
) -> Result<usize, Box<dyn Error>> {
    let projection = descriptor_overlay_projection(candidate, descriptors, viewport)?;
    if projection.targets.len() != usize::from(ENTRIES) {
        return Err("reference projection omitted a target".into());
    }
    let command_count = projection.commands.len();
    let snapshot = output_scene_snapshot_from_committed_in_view(
        OUTPUT,
        9,
        viewport,
        &[],
        CompositorDisplayList {
            output: OUTPUT,
            commands: projection.commands,
        },
        None,
    )?;
    let plans = build_output_head_plans(&snapshot, targets)?;
    if plans.len() != HEADS || plans[0].native_size == plans[1].native_size {
        return Err("reference projection did not produce two unequal head plans".into());
    }
    for plan in &plans {
        lower_head_composition_plan_with_caches(plan, &[], indicator_cache, text_cache)?;
    }
    Ok(command_count)
}

fn reference_input() -> (ChromeDescriptorTable, DescriptorOverlayCandidate) {
    let mut descriptors = ChromeDescriptorTable::default();
    let mut entries = Vec::with_capacity(usize::from(ENTRIES));
    for slot in 1..=ENTRIES {
        let surface = SurfaceId::new(u32::from(slot), 1);
        let generation = 100 + u64::from(slot);
        descriptors.upsert(ChromeDescriptor {
            surface,
            label: Some(DisplayLabel {
                text: format!("Reference window {slot}"),
                redacted: true,
            }),
            icon: None,
            trust_level: if slot % 2 == 0 {
                TrustLevel::Trusted
            } else {
                TrustLevel::Isolated
            },
            attention: if slot == ENTRIES {
                AttentionState::Critical
            } else {
                AttentionState::None
            },
            generation,
        });
        entries.push(DescriptorOverlayEntry {
            slot,
            surface,
            descriptor_generation: generation,
            action: ToplevelActionCapabilityRef {
                token: 1_000 + u64::from(slot),
                issuer_epoch: 3,
                issuer_revocation_epoch: 4,
                recipient_epoch: 5,
                target_slot: slot,
                target_generation: generation,
            },
        });
    }
    (
        descriptors,
        DescriptorOverlayCandidate {
            projection: 7,
            generation: 11,
            output: OUTPUT,
            broker_epoch: 3,
            broker_revocation_epoch: 4,
            shell_session_epoch: 5,
            selected_slot: Some(1),
            entries,
        },
    )
}

fn target(head: u64, width: i32, height: i32) -> HeadRenderTarget {
    HeadRenderTarget {
        head: RenderHeadId::from_raw(head),
        output: OUTPUT,
        target_generation: 2,
        native_size: Size { width, height },
        scale: 1,
        refresh_millihz: 60_000,
        transform: OutputTransform::Normal,
        mapping: OutputHeadMapping::Fit,
    }
}
