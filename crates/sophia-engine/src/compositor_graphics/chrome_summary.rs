use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompositorChromeSummary {
    pub generation: u64,
    pub frames: usize,
    pub focused_frames: usize,
    pub unfocused_frames: usize,
    pub focus_rings: usize,
    pub primitives: usize,
    pub clearance: i32,
}

pub fn compositor_chrome_summary(
    display_list: &CompositorDisplayList,
    focused_surface: Option<SurfaceId>,
) -> CompositorChromeSummary {
    let mut summary = CompositorChromeSummary {
        generation: 0xcbf2_9ce4_8422_2325,
        frames: 0,
        focused_frames: 0,
        unfocused_frames: 0,
        focus_rings: 0,
        primitives: 0,
        clearance: 0,
    };
    for border in display_list.borders() {
        let CompositorNodeId::SurfaceChrome { surface, role } = border.node;
        match role {
            SurfaceChromeRole::Frame => {
                summary.frames = summary.frames.saturating_add(1);
                if focused_surface == Some(surface) {
                    summary.focused_frames = summary.focused_frames.saturating_add(1);
                } else {
                    summary.unfocused_frames = summary.unfocused_frames.saturating_add(1);
                }
            }
            SurfaceChromeRole::FocusRing => {
                summary.focus_rings = summary.focus_rings.saturating_add(1);
            }
            SurfaceChromeRole::FloatingOutline => {}
        }
        summary.primitives = summary.primitives.saturating_add(
            compositor_border_bands(border)
                .into_iter()
                .filter(|band| !band.geometry.is_empty())
                .count(),
        );
        if role != SurfaceChromeRole::FloatingOutline {
            summary.clearance = summary
                .clearance
                .max(border.inner.x.saturating_sub(border.outer.x))
                .max(border.inner.y.saturating_sub(border.outer.y));
        }
        for byte in surface
            .index()
            .to_le_bytes()
            .into_iter()
            .chain(border.generation.to_le_bytes())
            .chain([role as u8])
        {
            summary.generation =
                (summary.generation ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    summary.generation = summary.generation.max(1);
    summary
}
