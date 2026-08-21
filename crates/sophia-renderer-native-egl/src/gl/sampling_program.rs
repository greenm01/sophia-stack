use crate::NativeCompositionSampling;

/// Which compiled program serves a draw.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompositionProgram {
    /// One texture fetch, no reconstruction. Exact draws and the fallback.
    Direct,
    /// The Catmull-Rom kernel, working in linear light.
    Reconstruction,
}

/// Everything a draw needs decided about how it samples, decided together.
///
/// The program and the texture filter were previously chosen in two places: the
/// filter at the call site from the requested sampling, and the program inside
/// the draw from whether it had compiled. Nothing made them agree. That was
/// harmless only while the filter for a reconstructed draw was `LINEAR` anyway;
/// it stops being harmless the moment reconstruction has to see unfiltered
/// texels, because a hardware bilinear applied before the shader runs would mix
/// gamma-encoded bytes behind its back and quietly undo the correction. They are
/// one decision now, taken once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompositionDrawPlan {
    pub program: CompositionProgram,
    pub effective: NativeCompositionSampling,
    pub status: &'static str,
    pub texture_filter: u32,
}

/// Resolve a requested sampling against what the pipeline actually has.
///
/// `NEAREST` for every reconstructed draw is the load-bearing part: the shader
/// gathers its own 4x4 footprint at texel centres and needs the texels as
/// stored. `LINEAR` survives only on the fallback, where no shader is running
/// and the hardware filter is the whole of the reconstruction.
pub(crate) const fn composition_draw_plan(
    requested: NativeCompositionSampling,
    reconstruction_available: bool,
) -> CompositionDrawPlan {
    match requested {
        NativeCompositionSampling::ExactNearest => CompositionDrawPlan {
            program: CompositionProgram::Direct,
            effective: NativeCompositionSampling::ExactNearest,
            status: "active",
            texture_filter: glow::NEAREST,
        },
        NativeCompositionSampling::SharpDownscale | NativeCompositionSampling::SharpUpscale => {
            if reconstruction_available {
                CompositionDrawPlan {
                    program: CompositionProgram::Reconstruction,
                    effective: requested,
                    status: "active",
                    texture_filter: glow::NEAREST,
                }
            } else {
                CompositionDrawPlan {
                    program: CompositionProgram::Direct,
                    effective: NativeCompositionSampling::LinearFallback,
                    status: "fallback",
                    texture_filter: glow::LINEAR,
                }
            }
        }
        // A caller cannot ask for the degraded path; `native_composition_sampling`
        // never yields it. Reaching here would mean an effective value was fed
        // back in as a request, so it resolves to itself rather than inventing a
        // better draw than the one that was asked for.
        NativeCompositionSampling::LinearFallback => CompositionDrawPlan {
            program: CompositionProgram::Direct,
            effective: NativeCompositionSampling::LinearFallback,
            status: "fallback",
            texture_filter: glow::LINEAR,
        },
    }
}

/// Which evidence bit a draw claims, so one line per distinct case is logged.
///
/// The sampling telemetry is first-occurrence evidence rather than a per-frame
/// record, because a readback-free log line still costs something on every draw
/// of every frame. The index pairs the requested class with the alpha mode, so a
/// premultiplied fallback and an opaque one are both seen.
pub(crate) const fn sampling_evidence_index(
    requested: NativeCompositionSampling,
    status_is_fallback: bool,
    premultiplied: bool,
) -> usize {
    let class = match requested {
        NativeCompositionSampling::ExactNearest => 0,
        NativeCompositionSampling::SharpDownscale if !status_is_fallback => 2,
        NativeCompositionSampling::SharpDownscale => 4,
        NativeCompositionSampling::SharpUpscale if !status_is_fallback => 6,
        NativeCompositionSampling::SharpUpscale => 8,
        NativeCompositionSampling::LinearFallback => 10,
    };
    class + if premultiplied { 1 } else { 0 }
}
