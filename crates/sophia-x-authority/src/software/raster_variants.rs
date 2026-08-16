use std::collections::{BTreeMap, BTreeSet};

use sophia_protocol::{
    MAX_SURFACE_CONTENT_VARIANTS, Rect, Region, SURFACE_CONTENT_DENSITY_1X_MILLIS, Size,
    SurfaceContentFidelity, SurfaceContentSet, SurfaceContentVariant, SurfaceRasterClass,
    SurfaceRasterRequirements, SurfaceRasterTransform,
};

use crate::{X_GX_COPY, XFontFace, XGraphicsContextValues, XResourceId};

use super::{
    X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888, X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES,
    XAuthorityCpuBufferSnapshot, XAuthorityCpuBufferUpdate, draw_line, draw_rectangle_outline,
    fill_rect, set_pixel,
};

pub(crate) const X_AUTHORITY_RASTER_JOURNAL_MAX_COMMANDS: usize = 4_096;
pub(crate) const X_AUTHORITY_RASTER_JOURNAL_MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct XRasterPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct XOwnedTextDraw {
    pub x: i32,
    pub baseline: i32,
    pub text: Vec<u8>,
    pub image: bool,
    pub font: XFontFace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum XAuthorityRasterCommand {
    Paint {
        rects: Vec<Rect>,
        gc: XGraphicsContextValues,
    },
    Clear {
        rect: Rect,
        pixel: u32,
    },
    Lines {
        points: Vec<XRasterPoint>,
        gc: XGraphicsContextValues,
    },
    Rectangles {
        rectangles: Vec<Rect>,
        gc: XGraphicsContextValues,
    },
    Text {
        draws: Vec<XOwnedTextDraw>,
        gc: XGraphicsContextValues,
    },
    CopyArea {
        source: Rect,
        destination_x: i32,
        destination_y: i32,
        gc: XGraphicsContextValues,
    },
    Unsupported,
}

impl XAuthorityRasterCommand {
    pub(crate) fn translated(mut self, x: i32, y: i32) -> Self {
        let translate_rect = |rect: &mut Rect| {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        };
        match &mut self {
            Self::Paint { rects, gc } => {
                rects.iter_mut().for_each(translate_rect);
                translate_gc_clip(gc, x, y);
            }
            Self::Clear { rect, .. } => translate_rect(rect),
            Self::Lines { points, gc } => {
                for point in points {
                    point.x = point.x.saturating_add(x);
                    point.y = point.y.saturating_add(y);
                }
                translate_gc_clip(gc, x, y);
            }
            Self::Rectangles { rectangles, gc } => {
                rectangles.iter_mut().for_each(translate_rect);
                translate_gc_clip(gc, x, y);
            }
            Self::Text { draws, gc } => {
                for draw in draws {
                    draw.x = draw.x.saturating_add(x);
                    draw.baseline = draw.baseline.saturating_add(y);
                }
                translate_gc_clip(gc, x, y);
            }
            Self::CopyArea {
                source,
                destination_x,
                destination_y,
                gc,
            } => {
                translate_rect(source);
                *destination_x = destination_x.saturating_add(x);
                *destination_y = destination_y.saturating_add(y);
                translate_gc_clip(gc, x, y);
            }
            Self::Unsupported => {}
        }
        self
    }

    fn payload_bytes(&self) -> usize {
        match self {
            Self::Paint { rects, gc } => {
                rects.len().saturating_mul(size_of::<Rect>()) + gc_bytes(gc)
            }
            Self::Clear { .. } => size_of::<Rect>() + size_of::<u32>(),
            Self::Lines { points, gc } => {
                points.len().saturating_mul(size_of::<XRasterPoint>()) + gc_bytes(gc)
            }
            Self::Rectangles { rectangles, gc } => {
                rectangles.len().saturating_mul(size_of::<Rect>()) + gc_bytes(gc)
            }
            Self::Text { draws, gc } => {
                draws.iter().map(|draw| draw.text.len()).sum::<usize>() + gc_bytes(gc)
            }
            Self::CopyArea { gc, .. } => size_of::<Rect>() + gc_bytes(gc),
            Self::Unsupported => 0,
        }
    }

    fn is_full_opaque_copy_clear(&self, logical_size: Size) -> bool {
        matches!(
            self,
            Self::Clear { rect, .. }
                if rect.x <= 0
                    && rect.y <= 0
                    && rect.x.saturating_add(rect.width) >= logical_size.width
                    && rect.y.saturating_add(rect.height) >= logical_size.height
        )
    }
}

fn gc_bytes(gc: &XGraphicsContextValues) -> usize {
    size_of::<XGraphicsContextValues>()
        .saturating_add(gc.clip_rectangles.len().saturating_mul(size_of::<Rect>()))
}

fn translate_gc_clip(gc: &mut XGraphicsContextValues, x: i32, y: i32) {
    gc.clip_x_origin = i16::try_from(i32::from(gc.clip_x_origin).saturating_add(x))
        .unwrap_or(if x < 0 { i16::MIN } else { i16::MAX });
    gc.clip_y_origin = i16::try_from(i32::from(gc.clip_y_origin).saturating_add(y))
        .unwrap_or(if y < 0 { i16::MIN } else { i16::MAX });
}

#[derive(Clone, Debug)]
struct VariantBacking {
    variant: u32,
    snapshot: XAuthorityCpuBufferSnapshot,
}

#[derive(Clone, Debug)]
struct SurfaceRasterState {
    logical_size: Size,
    journal: Vec<XAuthorityRasterCommand>,
    journal_payload_bytes: usize,
    replayable: bool,
    next_variant: u32,
    required: BTreeSet<SurfaceRasterClass>,
    variants: BTreeMap<SurfaceRasterClass, VariantBacking>,
}

impl SurfaceRasterState {
    fn new(logical_size: Size) -> Self {
        Self {
            logical_size,
            journal: Vec::new(),
            journal_payload_bytes: 0,
            replayable: true,
            next_variant: 2,
            required: BTreeSet::new(),
            variants: BTreeMap::new(),
        }
    }
}

/// Authority-private semantic journal and native-density presentation stores.
/// Canonical X11 drawable pixels remain in `XSoftwareBufferStore`; this owner
/// never exposes a connector, CRTC, or client XID to Engine.
#[derive(Debug)]
pub(crate) struct XAuthorityRasterStore {
    next_handle: u64,
    surfaces: BTreeMap<XResourceId, SurfaceRasterState>,
}

impl Default for XAuthorityRasterStore {
    fn default() -> Self {
        Self {
            // Derived handles occupy a disjoint authority-owned range.
            next_handle: 1_u64 << 63,
            surfaces: BTreeMap::new(),
        }
    }
}

impl XAuthorityRasterStore {
    pub(crate) fn remove(&mut self, presentation: XResourceId) {
        self.surfaces.remove(&presentation);
    }

    pub(crate) fn record(
        &mut self,
        presentation: XResourceId,
        logical_size: Size,
        command: XAuthorityRasterCommand,
    ) -> Vec<XAuthorityCpuBufferUpdate> {
        let state = self
            .surfaces
            .entry(presentation)
            .or_insert_with(|| SurfaceRasterState::new(logical_size));
        if state.logical_size != logical_size {
            *state = SurfaceRasterState::new(logical_size);
        }
        if command.is_full_opaque_copy_clear(logical_size) {
            state.journal.clear();
            state.journal_payload_bytes = 0;
            state.replayable = true;
        }
        let payload = command.payload_bytes();
        if matches!(command, XAuthorityRasterCommand::Unsupported)
            || state.journal.len() >= X_AUTHORITY_RASTER_JOURNAL_MAX_COMMANDS
            || state.journal_payload_bytes.saturating_add(payload)
                > X_AUTHORITY_RASTER_JOURNAL_MAX_PAYLOAD_BYTES
        {
            state.replayable = false;
            state.journal.clear();
            state.journal_payload_bytes = 0;
            state.variants.clear();
            return Vec::new();
        }
        state.journal_payload_bytes = state.journal_payload_bytes.saturating_add(payload);
        state.journal.push(command.clone());
        if !state.replayable {
            return Vec::new();
        }
        let mut updates = Vec::new();
        for (class, backing) in &mut state.variants {
            apply_command(&mut backing.snapshot, *class, &command);
            backing.snapshot.generation = backing.snapshot.generation.saturating_add(1);
            updates.push(XAuthorityCpuBufferUpdate::Replace(backing.snapshot.clone()));
        }
        updates
    }

    pub(crate) fn satisfy(
        &mut self,
        presentation: XResourceId,
        requirements: &SurfaceRasterRequirements,
        canonical_bytes: usize,
    ) -> Result<Vec<XAuthorityCpuBufferUpdate>, &'static str> {
        requirements
            .validate()
            .map_err(|_| "invalid surface raster requirements")?;
        let state = self
            .surfaces
            .entry(presentation)
            .or_insert_with(|| SurfaceRasterState::new(requirements.logical_extent));
        if state.logical_size != requirements.logical_extent {
            return Err("surface raster requirement logical extent changed");
        }
        if !state.replayable {
            return Ok(Vec::new());
        }

        // Requirement satisfaction is atomic. In particular, a four-class
        // non-1x request cannot partially replace retained variants and leak
        // unpublished backing handles before the caller selects fallback.
        let mut candidate = state.clone();
        let mut next_handle = self.next_handle;
        candidate.required = requirements.classes.iter().copied().collect();
        candidate
            .variants
            .retain(|class, _| candidate.required.contains(class));
        let mut projected_bytes = canonical_bytes.saturating_add(
            candidate
                .variants
                .values()
                .map(|variant| variant.snapshot.bytes.len())
                .sum::<usize>(),
        );
        let mut updates = Vec::new();
        for class in &candidate.required {
            if class.transform != SurfaceRasterTransform::Normal
                || class.density_millis == SURFACE_CONTENT_DENSITY_1X_MILLIS
                || candidate.variants.contains_key(class)
            {
                continue;
            }
            // The canonical 1x X11 backing always occupies one protocol
            // variant. A requirement may legally name the protocol-wide
            // maximum without including 1x, so fail it as sampled fallback
            // instead of constructing an over-capacity content set.
            if candidate.variants.len() >= MAX_SURFACE_CONTENT_VARIANTS.saturating_sub(1) {
                continue;
            }
            let size = projected_size(candidate.logical_size, class.density_millis)
                .ok_or("surface raster requirement size overflow")?;
            let bytes =
                buffer_bytes(size).ok_or("surface raster requirement exceeds backing bound")?;
            if projected_bytes.saturating_add(bytes) > X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
                continue;
            }
            projected_bytes = projected_bytes.saturating_add(bytes);
            let handle = next_handle.max(1_u64 << 63);
            next_handle = handle.saturating_add(1).max(1_u64 << 63);
            let mut snapshot = XAuthorityCpuBufferSnapshot {
                handle,
                drawable: presentation,
                size,
                stride: u32::try_from(usize::try_from(size.width).unwrap_or(0).saturating_mul(4))
                    .map_err(|_| "surface raster stride overflow")?,
                format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                generation: 1,
                bytes: vec![0; bytes],
            };
            for command in &candidate.journal {
                apply_command(&mut snapshot, *class, command);
            }
            let variant = candidate.next_variant.max(2);
            candidate.next_variant = variant.saturating_add(1).max(2);
            updates.push(XAuthorityCpuBufferUpdate::Replace(snapshot.clone()));
            candidate
                .variants
                .insert(*class, VariantBacking { variant, snapshot });
        }
        let all_satisfied = candidate.required.iter().all(|class| {
            (class.transform == SurfaceRasterTransform::Normal
                && class.density_millis == SURFACE_CONTENT_DENSITY_1X_MILLIS)
                || candidate.variants.contains_key(class)
        });
        if !all_satisfied {
            return Ok(Vec::new());
        }
        *state = candidate;
        self.next_handle = next_handle;
        Ok(updates)
    }

    pub(crate) fn content_set(
        &self,
        presentation: XResourceId,
        canonical: &XAuthorityCpuBufferSnapshot,
    ) -> SurfaceContentSet {
        let mut variants = vec![SurfaceContentVariant {
            variant: 1,
            source: sophia_protocol::BufferSource::CpuBuffer {
                handle: canonical.handle,
            },
            pixel_size: canonical.size,
            density_millis: SURFACE_CONTENT_DENSITY_1X_MILLIS,
            transform: SurfaceRasterTransform::Normal,
            fidelity: SurfaceContentFidelity::AuthorityRaster,
            damage: full_damage(canonical.size),
        }];
        if let Some(state) = self.surfaces.get(&presentation) {
            variants.extend(
                state
                    .variants
                    .iter()
                    .take(MAX_SURFACE_CONTENT_VARIANTS.saturating_sub(1))
                    .map(|(class, backing)| SurfaceContentVariant {
                        variant: backing.variant,
                        source: sophia_protocol::BufferSource::CpuBuffer {
                            handle: backing.snapshot.handle,
                        },
                        pixel_size: backing.snapshot.size,
                        density_millis: class.density_millis,
                        transform: class.transform,
                        fidelity: SurfaceContentFidelity::AuthorityRaster,
                        damage: full_damage(backing.snapshot.size),
                    }),
            );
        }
        SurfaceContentSet::new(canonical.size, variants)
            .expect("authority raster store preserves content-set invariants")
    }
}

fn full_damage(size: Size) -> Region {
    Region::single(Rect {
        x: 0,
        y: 0,
        width: size.width,
        height: size.height,
    })
}

fn projected_size(logical: Size, density: u32) -> Option<Size> {
    let edge = |value: i32| {
        i32::try_from(
            u64::try_from(value)
                .ok()?
                .checked_mul(u64::from(density))?
                .checked_add(999)?
                / 1_000,
        )
        .ok()
    };
    Some(Size {
        width: edge(logical.width)?,
        height: edge(logical.height)?,
    })
}

fn buffer_bytes(size: Size) -> Option<usize> {
    usize::try_from(size.width)
        .ok()?
        .checked_mul(usize::try_from(size.height).ok()?)?
        .checked_mul(4)
        .filter(|bytes| *bytes <= X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES)
}

fn floor_edge(value: i32, density: u32) -> i32 {
    let scaled = i64::from(value).saturating_mul(i64::from(density));
    let quotient = scaled.div_euclid(1_000);
    i32::try_from(quotient).unwrap_or(if quotient < 0 { i32::MIN } else { i32::MAX })
}

fn ceil_edge(value: i32, density: u32) -> i32 {
    let scaled = i64::from(value).saturating_mul(i64::from(density));
    let quotient = scaled.saturating_add(999).div_euclid(1_000);
    i32::try_from(quotient).unwrap_or(if quotient < 0 { i32::MIN } else { i32::MAX })
}

fn project_rect(rect: Rect, density: u32) -> Rect {
    let left = floor_edge(rect.x, density);
    let top = floor_edge(rect.y, density);
    let right = ceil_edge(rect.x.saturating_add(rect.width), density);
    let bottom = ceil_edge(rect.y.saturating_add(rect.height), density);
    Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

fn projected_gc(gc: &XGraphicsContextValues, density: u32) -> XGraphicsContextValues {
    let mut projected = gc.clone();
    projected.line_width =
        u16::try_from(ceil_edge(i32::from(gc.line_width.max(1)), density).max(1))
            .unwrap_or(u16::MAX);
    projected.clip_x_origin = i16::try_from(floor_edge(i32::from(gc.clip_x_origin), density))
        .unwrap_or(if gc.clip_x_origin < 0 {
            i16::MIN
        } else {
            i16::MAX
        });
    projected.clip_y_origin = i16::try_from(floor_edge(i32::from(gc.clip_y_origin), density))
        .unwrap_or(if gc.clip_y_origin < 0 {
            i16::MIN
        } else {
            i16::MAX
        });
    projected.clip_rectangles = gc
        .clip_rectangles
        .iter()
        .map(|rect| project_rect(*rect, density))
        .collect();
    projected
}

fn apply_command(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    class: SurfaceRasterClass,
    command: &XAuthorityRasterCommand,
) {
    let density = class.density_millis;
    match command {
        XAuthorityRasterCommand::Paint { rects, gc } => {
            let gc = projected_gc(gc, density);
            for rect in rects {
                fill_rect(snapshot, project_rect(*rect, density), gc.foreground, &gc);
            }
        }
        XAuthorityRasterCommand::Clear { rect, pixel } => fill_rect(
            snapshot,
            project_rect(*rect, density),
            *pixel,
            &XGraphicsContextValues::default(),
        ),
        XAuthorityRasterCommand::Lines { points, gc } => {
            let gc = projected_gc(gc, density);
            for pair in points.windows(2) {
                draw_line(
                    snapshot,
                    crate::XPoint {
                        x: i16::try_from(floor_edge(pair[0].x, density)).unwrap_or(i16::MAX),
                        y: i16::try_from(floor_edge(pair[0].y, density)).unwrap_or(i16::MAX),
                    },
                    crate::XPoint {
                        x: i16::try_from(floor_edge(pair[1].x, density)).unwrap_or(i16::MAX),
                        y: i16::try_from(floor_edge(pair[1].y, density)).unwrap_or(i16::MAX),
                    },
                    i32::from(gc.line_width.max(1)),
                    &gc,
                );
            }
        }
        XAuthorityRasterCommand::Rectangles { rectangles, gc } => {
            let gc = projected_gc(gc, density);
            for rectangle in rectangles {
                draw_rectangle_outline(
                    snapshot,
                    project_rect(*rectangle, density),
                    i32::from(gc.line_width.max(1)),
                    &gc,
                );
            }
        }
        XAuthorityRasterCommand::Text { draws, gc } => {
            for draw in draws {
                draw_projected_text(snapshot, density, draw, gc);
            }
        }
        XAuthorityRasterCommand::CopyArea {
            source,
            destination_x,
            destination_y,
            gc,
        } => copy_projected_area(
            snapshot,
            project_rect(*source, density),
            floor_edge(*destination_x, density),
            floor_edge(*destination_y, density),
            &projected_gc(gc, density),
        ),
        XAuthorityRasterCommand::Unsupported => {}
    }
}

fn draw_projected_text(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    density: u32,
    draw: &XOwnedTextDraw,
    gc: &XGraphicsContextValues,
) {
    let top = draw.baseline.saturating_sub(draw.font.ascent());
    let width = i32::try_from(draw.text.len())
        .unwrap_or(i32::MAX)
        .saturating_mul(draw.font.width());
    let mut raster_gc = projected_gc(gc, density);
    if draw.image {
        raster_gc.function = X_GX_COPY;
        raster_gc.fill_style = 0;
        fill_rect(
            snapshot,
            project_rect(
                Rect {
                    x: draw.x,
                    y: top,
                    width,
                    height: draw.font.ascent().saturating_add(draw.font.descent()),
                },
                density,
            ),
            gc.background,
            &raster_gc,
        );
    }
    for (index, byte) in draw.text.iter().copied().enumerate() {
        let cell_x = draw.x.saturating_add(
            i32::try_from(index)
                .unwrap_or(i32::MAX)
                .saturating_mul(draw.font.width()),
        );
        draw_coverage_glyph(
            snapshot,
            density,
            cell_x,
            top,
            byte,
            gc.foreground,
            draw.font,
            &raster_gc,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_coverage_glyph(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    density: u32,
    cell_x: i32,
    cell_y: i32,
    byte: u8,
    pixel: u32,
    font: XFontFace,
    gc: &XGraphicsContextValues,
) {
    let bounds = project_rect(
        Rect {
            x: cell_x,
            y: cell_y,
            width: font.width(),
            height: font.ascent().saturating_add(font.descent()),
        },
        density,
    );
    let rows = font.glyph_rows(byte);
    for y in bounds.y..bounds.y.saturating_add(bounds.height) {
        for x in bounds.x..bounds.x.saturating_add(bounds.width) {
            let mut area = 0_i64;
            for (row, bits) in rows.iter().copied().enumerate() {
                for column in 0..6_i32 {
                    if bits & (1 << (5 - column)) == 0 {
                        continue;
                    }
                    let sx = cell_x.saturating_add(column);
                    let sy = cell_y.saturating_add(i32::try_from(row).unwrap_or(0));
                    let overlap_x = overlap_scaled(x, sx, density);
                    let overlap_y = overlap_scaled(y, sy, density);
                    area = area.saturating_add(overlap_x.saturating_mul(overlap_y));
                }
            }
            let coverage =
                u8::try_from(area.saturating_mul(255).saturating_add(500_000) / 1_000_000)
                    .unwrap_or(u8::MAX);
            if coverage == 0 {
                continue;
            }
            if gc.function == X_GX_COPY {
                blend_copy_pixel(snapshot, x, y, pixel, coverage, gc);
            } else if coverage >= 128 {
                set_pixel(snapshot, x, y, pixel, gc);
            }
        }
    }
}

fn overlap_scaled(destination: i32, source: i32, density: u32) -> i64 {
    let destination_left = i64::from(destination).saturating_mul(1_000);
    let destination_right = destination_left.saturating_add(1_000);
    let source_left = i64::from(source).saturating_mul(i64::from(density));
    let source_right = source_left.saturating_add(i64::from(density));
    destination_right
        .min(source_right)
        .saturating_sub(destination_left.max(source_left))
        .max(0)
}

fn blend_copy_pixel(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    x: i32,
    y: i32,
    source: u32,
    coverage: u8,
    gc: &XGraphicsContextValues,
) {
    let Ok(xu) = usize::try_from(x) else { return };
    let Ok(yu) = usize::try_from(y) else { return };
    let Ok(stride) = usize::try_from(snapshot.stride) else {
        return;
    };
    let Some(offset) = yu
        .checked_mul(stride)
        .and_then(|row| row.checked_add(xu.saturating_mul(4)))
    else {
        return;
    };
    let Some(bytes) = snapshot.bytes.get(offset..offset.saturating_add(4)) else {
        return;
    };
    let destination = u32::from_le_bytes(bytes.try_into().unwrap_or([0; 4]));
    let alpha = u32::from(coverage);
    let blend = |shift: u32| -> u32 {
        let source: u32 = (source >> shift) & 0xff;
        let destination: u32 = (destination >> shift) & 0xff;
        (source
            .saturating_mul(alpha)
            .saturating_add(destination.saturating_mul(255 - alpha))
            .saturating_add(127))
            / 255
    };
    let pixel = blend(16) << 16 | blend(8) << 8 | blend(0);
    set_pixel(snapshot, x, y, pixel, gc);
}

fn copy_projected_area(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    source: Rect,
    destination_x: i32,
    destination_y: i32,
    gc: &XGraphicsContextValues,
) {
    let previous = snapshot.clone();
    let Ok(stride) = usize::try_from(previous.stride) else {
        return;
    };
    for y in 0..source.height.max(0) {
        for x in 0..source.width.max(0) {
            let sx = source.x.saturating_add(x);
            let sy = source.y.saturating_add(y);
            let Ok(sxu) = usize::try_from(sx) else {
                continue;
            };
            let Ok(syu) = usize::try_from(sy) else {
                continue;
            };
            let Some(offset) = syu
                .checked_mul(stride)
                .and_then(|row| row.checked_add(sxu.saturating_mul(4)))
            else {
                continue;
            };
            let Some(bytes) = previous.bytes.get(offset..offset.saturating_add(4)) else {
                continue;
            };
            set_pixel(
                snapshot,
                destination_x.saturating_add(x),
                destination_y.saturating_add(y),
                u32::from_le_bytes(bytes.try_into().unwrap_or([0; 4])),
                gc,
            );
        }
    }
}
