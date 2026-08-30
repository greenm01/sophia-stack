use std::collections::BTreeMap;
use std::sync::Arc;

use sophia_protocol::{Rect, Size};

use crate::{XFontFace, XGraphicsContextValues, XPoint, XResourceId};

mod raster_ops;
mod raster_replay;
mod raster_variants;
mod update;

use raster_ops::{
    copy_buffer_region, copy_xrgb8888, draw_fixed_glyph, draw_line, draw_rectangle_outline,
    fill_rect, point_bounds, rectangle_outline_bounds, set_pixel,
};
pub(crate) use raster_variants::{
    XAuthorityRasterCommand, XAuthorityRasterStore, XOwnedTextDraw, XRasterPoint,
    XRasterSatisfyOutcome, XRasterUnsupportedKind,
};
pub use raster_variants::{XPutImageSemantics, XRasterFallbackCause};
pub use update::{
    X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS, XAuthorityCpuBufferPatch, XAuthorityCpuBufferPatchBatch,
    XAuthorityCpuBufferPatchRegion, XAuthorityCpuBufferSnapshot, XAuthorityCpuBufferUpdate,
    XAuthorityCpuDrawResult,
};
use update::{packed_patch_region, patch_is_representable};

pub const X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
pub const X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct XSoftwareBufferStore {
    next_handle: u64,
    buffers: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
    presentations: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
}

impl XSoftwareBufferStore {
    /// Whether this drawable already holds pixels written through the CPU path.
    ///
    /// Distinct from a presentation snapshot, which only windows acquire: a
    /// pixmap's pixels live here from its first upload.
    pub(crate) fn has_cpu_backing(&self, drawable: XResourceId) -> bool {
        self.buffers.contains_key(&drawable)
    }

    pub(crate) fn presentation_snapshot(
        &self,
        drawable: XResourceId,
    ) -> Option<&XAuthorityCpuBufferSnapshot> {
        self.presentations.get(&drawable)
    }
    pub fn remove(&mut self, drawable: XResourceId) -> Option<XAuthorityCpuBufferSnapshot> {
        self.presentations.remove(&drawable);
        self.buffers.remove(&drawable)
    }

    pub fn present_window_damage(
        &mut self,
        presentation: XResourceId,
        presentation_size: Size,
        source: XResourceId,
        source_offset_x: i32,
        source_offset_y: i32,
        damage: &[Rect],
    ) -> Option<XAuthorityCpuBufferUpdate> {
        let (source_drawable, source_size) = {
            let source_buffer = self.buffers.get(&source)?;
            (source_buffer.drawable, source_buffer.size)
        };
        if presentation_size.width <= 0 || presentation_size.height <= 0 {
            return None;
        }
        let source_extent = Size {
            width: source_offset_x
                .saturating_add(source_size.width)
                .clamp(1, presentation_size.width),
            height: source_offset_y
                .saturating_add(source_size.height)
                .clamp(1, presentation_size.height),
        };
        let desired_size = if source_drawable == presentation {
            presentation_size
        } else {
            self.presentations
                .get(&presentation)
                .map(|buffer| Size {
                    width: buffer
                        .size
                        .width
                        .max(source_extent.width)
                        .min(presentation_size.width),
                    height: buffer
                        .size
                        .height
                        .max(source_extent.height)
                        .min(presentation_size.height),
                })
                .unwrap_or(source_extent)
        };
        let replace = self
            .presentations
            .get(&presentation)
            .is_none_or(|buffer| buffer.size != desired_size);
        if replace {
            let previous = self.presentations.get(&presentation).cloned();
            let width = usize::try_from(desired_size.width).ok()?;
            let height = usize::try_from(desired_size.height).ok()?;
            let stride = width.checked_mul(4)?;
            let byte_len = stride.checked_mul(height)?;
            if width == 0 || height == 0 || byte_len > X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
                return None;
            }
            let handle = self.allocate_handle();
            let generation = self
                .presentations
                .get(&presentation)
                .map_or(0, |buffer| buffer.generation);
            self.presentations.insert(
                presentation,
                XAuthorityCpuBufferSnapshot {
                    handle,
                    drawable: presentation,
                    size: desired_size,
                    stride: u32::try_from(stride).ok()?,
                    format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                    generation,
                    bytes: Arc::new(vec![0; byte_len]),
                },
            );
            if let Some(previous) = previous
                && let Some(buffer) = self.presentations.get_mut(&presentation)
            {
                copy_buffer_region(
                    &previous,
                    buffer,
                    Rect {
                        x: 0,
                        y: 0,
                        width: previous.size.width,
                        height: previous.size.height,
                    },
                    0,
                    0,
                );
            }
        }
        let source = self.buffers.get(&source)?;
        let presentation_buffer = self.presentations.get_mut(&presentation)?;
        let mut presentation_damage = Vec::with_capacity(damage.len());
        for rect in damage {
            if let Some(rect) = copy_buffer_region(
                &source,
                presentation_buffer,
                *rect,
                source_offset_x,
                source_offset_y,
            ) {
                presentation_damage.push(rect);
            }
        }
        presentation_buffer.generation = presentation_buffer.generation.checked_add(1)?;
        // A busy client is not a reason to resend the window. The transport
        // carries at most 32 rectangles, and a damage list longer than that
        // used to fall back to replacing the whole presentation buffer -- which
        // is the common case for a browser, the one client whose buffers are
        // largest. Coalescing merges the list down to the bound instead, and
        // the merged cover is a superset of the damage, so the patch carries
        // pixels that are already correct in the buffer it is read from.
        //
        // The bound itself does not move. It is validated identically on both
        // sides of the wire, so raising it would have to move the encoder, both
        // guards, and the renderer's capacity refusal together.
        if replace {
            return Some(XAuthorityCpuBufferUpdate::Replace(
                presentation_buffer.clone(),
            ));
        }
        let presentation_damage =
            if presentation_damage.len() > X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS {
                let coalesced =
                    coalesce_damage(presentation_damage, X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS);
                // Past a point a merged cover stops being cheaper than the buffer.
                // Half the area is where it has lost the argument: the batch still
                // carries per-rectangle headers and the receiver still walks them,
                // for a saving that is no longer most of the frame.
                //
                // Only a coalesced list is measured. A short damage list is sent as
                // it stands whatever it covers, which is the behaviour every
                // existing caller and regression already depends on.
                let buffer_area = usize::try_from(presentation_buffer.size.width)
                    .ok()?
                    .saturating_mul(usize::try_from(presentation_buffer.size.height).ok()?);
                if coverage_area(&coalesced).saturating_mul(2) >= buffer_area {
                    return Some(XAuthorityCpuBufferUpdate::Replace(
                        presentation_buffer.clone(),
                    ));
                }
                coalesced
            } else {
                presentation_damage
            };
        let patches = presentation_damage
            .into_iter()
            .map(|rect| packed_patch_region(presentation_buffer, rect))
            .collect::<Option<Vec<_>>>()?;
        Some(XAuthorityCpuBufferUpdate::PatchBatch(
            XAuthorityCpuBufferPatchBatch {
                handle: presentation_buffer.handle,
                drawable: presentation_buffer.drawable,
                size: presentation_buffer.size,
                stride: presentation_buffer.stride,
                format: presentation_buffer.format,
                generation: presentation_buffer.generation,
                patches,
            },
        ))
    }

    pub fn paint_damage(
        &mut self,
        drawable: XResourceId,
        size: Size,
        damage: &[Rect],
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuDrawResult> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        for rect in damage {
            fill_rect(buffer, *rect, gc.foreground, gc);
        }
        finish_immutable_update(buffer, handle, replaced, union_rects(damage))
    }

    pub fn clear(
        &mut self,
        drawable: XResourceId,
        size: Size,
        rect: Rect,
        pixel: u32,
    ) -> Option<XAuthorityCpuDrawResult> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        fill_rect(buffer, rect, pixel, &XGraphicsContextValues::default());
        finish_immutable_update(buffer, handle, replaced, Some(rect))
    }

    pub fn draw_text(
        &mut self,
        drawable: XResourceId,
        size: Size,
        draws: &[XTextDraw<'_>],
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuDrawResult> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        let mut damage = Vec::with_capacity(draws.len());
        for draw in draws {
            if draw.text.is_empty() {
                continue;
            }
            let top = draw.baseline.saturating_sub(draw.font.ascent());
            let width = i32::try_from(draw.text.len())
                .unwrap_or(i32::MAX)
                .saturating_mul(draw.font.width());
            let draw_gc;
            let raster_gc = if draw.image {
                draw_gc = XGraphicsContextValues {
                    function: crate::X_GX_COPY,
                    fill_style: 0,
                    ..gc.clone()
                };
                &draw_gc
            } else {
                gc
            };
            if draw.image {
                fill_rect(
                    buffer,
                    Rect {
                        x: draw.x,
                        y: top,
                        width,
                        height: draw.font.ascent().saturating_add(draw.font.descent()),
                    },
                    gc.background,
                    raster_gc,
                );
            }
            for (index, byte) in draw.text.iter().copied().enumerate() {
                let cell_x = draw.x.saturating_add(
                    i32::try_from(index)
                        .unwrap_or(i32::MAX)
                        .saturating_mul(draw.font.width()),
                );
                draw_fixed_glyph(
                    buffer,
                    cell_x,
                    top,
                    byte,
                    gc.foreground,
                    draw.font,
                    raster_gc,
                );
            }
            damage.push(Rect {
                x: draw.x,
                y: top,
                width,
                height: draw.font.ascent().saturating_add(draw.font.descent()),
            });
        }
        finish_immutable_update(buffer, handle, replaced, union_rects(&damage))
    }

    pub fn put_image(
        &mut self,
        drawable: XResourceId,
        size: Size,
        destination: Rect,
        data: &[u8],
    ) -> Option<XAuthorityCpuDrawResult> {
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        copy_xrgb8888(buffer, destination, data);
        finish_immutable_update(buffer, handle, replaced, Some(destination))
    }

    pub fn ensure_image_backing(&mut self, drawable: XResourceId, size: Size) -> Option<()> {
        let handle = self.allocate_handle();
        self.ensure(drawable, size, handle).map(|_| ())
    }

    pub fn put_image_backing(
        &mut self,
        drawable: XResourceId,
        size: Size,
        destination: Rect,
        data: &[u8],
    ) -> Option<()> {
        let handle = self.allocate_handle();
        let (buffer, _) = self.ensure(drawable, size, handle)?;
        copy_xrgb8888(buffer, destination, data);
        buffer.generation = buffer.generation.checked_add(1)?;
        Some(())
    }

    pub fn image_region(&self, drawable: XResourceId, region: Rect) -> Option<Vec<u8>> {
        let width = usize::try_from(region.width).ok()?;
        let height = usize::try_from(region.height).ok()?;
        let byte_len = width.checked_mul(height)?.checked_mul(4)?;
        if byte_len > X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
            return None;
        }
        if width == 0 || height == 0 {
            return Some(Vec::new());
        }
        let mut image = Vec::new();
        image.try_reserve_exact(byte_len).ok()?;
        image.resize(byte_len, 0);
        let Some(buffer) = self.buffers.get(&drawable) else {
            return Some(image);
        };
        let source_width = usize::try_from(buffer.size.width).ok()?;
        let source_height = usize::try_from(buffer.size.height).ok()?;
        let source_stride = usize::try_from(buffer.stride).ok()?;
        let destination_stride = width.checked_mul(4)?;
        for row in 0..height {
            let source_y = region
                .y
                .checked_add(i32::try_from(row).ok()?)
                .and_then(|y| usize::try_from(y).ok());
            let Some(source_y) = source_y.filter(|y| *y < source_height) else {
                continue;
            };
            for column in 0..width {
                let source_x = region
                    .x
                    .checked_add(i32::try_from(column).ok()?)
                    .and_then(|x| usize::try_from(x).ok());
                let Some(source_x) = source_x.filter(|x| *x < source_width) else {
                    continue;
                };
                let source_offset = source_y
                    .checked_mul(source_stride)?
                    .checked_add(source_x.checked_mul(4)?)?;
                let destination_offset = row
                    .checked_mul(destination_stride)?
                    .checked_add(column.checked_mul(4)?)?;
                image
                    .get_mut(destination_offset..destination_offset.checked_add(4)?)?
                    .copy_from_slice(
                        buffer
                            .bytes
                            .get(source_offset..source_offset.checked_add(4)?)?,
                    );
            }
        }
        Some(image)
    }

    pub fn draw_lines(
        &mut self,
        drawable: XResourceId,
        size: Size,
        points: &[XPoint],
        gc: &XGraphicsContextValues,
    ) -> Option<XAuthorityCpuDrawResult> {
        let damage = point_bounds(points, gc.line_width)?;
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        let width = i32::from(gc.line_width.max(1));
        for pair in points.windows(2) {
            draw_line(buffer, pair[0], pair[1], width, gc);
        }
        finish_immutable_update(buffer, handle, replaced, Some(damage))
    }

    pub fn draw_rectangles(
        &mut self,
        drawable: XResourceId,
        size: Size,
        rectangles: &[Rect],
        gc: &XGraphicsContextValues,
    ) -> Option<(XAuthorityCpuDrawResult, Rect)> {
        let damage = rectangle_outline_bounds(rectangles, gc.line_width)?;
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(drawable, size, handle)?;
        let line_width = i32::from(gc.line_width.max(1));
        for rectangle in rectangles {
            draw_rectangle_outline(buffer, *rectangle, line_width, gc);
        }
        finish_immutable_update(buffer, handle, replaced, Some(damage))
            .map(|update| (update, damage))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn copy_area(
        &mut self,
        source: XResourceId,
        destination: XResourceId,
        destination_size: Size,
        source_rect: Rect,
        dst_x: i16,
        dst_y: i16,
        gc: &XGraphicsContextValues,
    ) -> Option<(XAuthorityCpuDrawResult, Rect)> {
        let source = self.buffers.get(&source)?.clone();
        let handle = self.allocate_handle();
        let (buffer, replaced) = self.ensure(destination, destination_size, handle)?;
        let source_width = source.size.width;
        let source_height = source.size.height;
        let destination_width = destination_size.width;
        let destination_height = destination_size.height;
        let requested_width = source_rect.width.max(0);
        let requested_height = source_rect.height.max(0);
        let offset_left = 0
            .max(source_rect.x.saturating_neg())
            .max(i32::from(dst_x).saturating_neg());
        let offset_top = 0
            .max(source_rect.y.saturating_neg())
            .max(i32::from(dst_y).saturating_neg());
        let offset_right = requested_width
            .min(source_width.saturating_sub(source_rect.x))
            .min(destination_width.saturating_sub(i32::from(dst_x)));
        let offset_bottom = requested_height
            .min(source_height.saturating_sub(source_rect.y))
            .min(destination_height.saturating_sub(i32::from(dst_y)));
        if offset_right <= offset_left || offset_bottom <= offset_top {
            return None;
        }
        let source_stride = usize::try_from(source.stride).ok()?;
        for y_offset in offset_top..offset_bottom {
            let source_y = usize::try_from(source_rect.y.saturating_add(y_offset)).ok()?;
            for x_offset in offset_left..offset_right {
                let source_x = usize::try_from(source_rect.x.saturating_add(x_offset)).ok()?;
                let offset = source_y
                    .saturating_mul(source_stride)
                    .saturating_add(source_x.saturating_mul(4));
                let pixel = u32::from_le_bytes(
                    source
                        .bytes
                        .get(offset..offset.saturating_add(4))?
                        .try_into()
                        .ok()?,
                );
                let target_x = i32::from(dst_x).saturating_add(x_offset);
                let target_y = i32::from(dst_y).saturating_add(y_offset);
                set_pixel(buffer, target_x, target_y, pixel, gc);
            }
        }
        let damage = Rect {
            x: i32::from(dst_x).saturating_add(offset_left),
            y: i32::from(dst_y).saturating_add(offset_top),
            width: offset_right.saturating_sub(offset_left),
            height: offset_bottom.saturating_sub(offset_top),
        };
        finish_immutable_update(buffer, handle, replaced, Some(damage))
            .map(|update| (update, damage))
    }

    fn ensure(
        &mut self,
        drawable: XResourceId,
        size: Size,
        handle: u64,
    ) -> Option<(&mut XAuthorityCpuBufferSnapshot, bool)> {
        let width = usize::try_from(size.width).ok()?;
        let height = usize::try_from(size.height).ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        let stride = width.checked_mul(4)?;
        let byte_len = stride.checked_mul(height)?;
        if byte_len > X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES {
            return None;
        }

        let replace = self
            .buffers
            .get(&drawable)
            .is_none_or(|buffer| buffer.size != size);
        if replace {
            let previous = self.buffers.get(&drawable);
            let generation = previous.map_or(0, |buffer| buffer.generation);
            self.buffers.insert(
                drawable,
                XAuthorityCpuBufferSnapshot {
                    handle,
                    drawable,
                    size,
                    stride: u32::try_from(stride).ok()?,
                    format: X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888,
                    generation,
                    bytes: Arc::new(vec![0; byte_len]),
                },
            );
        }
        self.buffers
            .get_mut(&drawable)
            .map(|buffer| (buffer, replace))
    }

    fn allocate_handle(&mut self) -> u64 {
        let handle = self.next_handle.max(1);
        self.next_handle = handle.saturating_add(1).max(1);
        handle
    }
}

/// Rebrand a drawable's snapshot after a core drawing operation.
///
/// The damage is still validated exactly as before. `patch_is_representable`
/// makes every refusal `packed_patch` makes about a rectangle -- an empty or
/// fully clipped rect, an unrepresentable offset, a snapshot too short for the
/// rows it claims -- and returning `None` here is what turns those into
/// `InvalidResource`. What is gone is the copy: the patch was built and dropped
/// without being read, and the snapshot was cloned whole for a caller that
/// wanted its handle.
///
/// The pixels reach Engine through `present_window_damage`, which composes this
/// drawable into its toplevel's presentation buffer and publishes that.
/// Merge a damage list down to at most `limit` rectangles.
///
/// Deterministic, because the wire tests replay a recorded sequence and compare
/// what came out: rectangles are ordered by row then column, split into `limit`
/// contiguous groups, and each group becomes its bounding box. Sorting by
/// position first is what makes the groups spatially close, so the boxes stay
/// near the damage rather than spanning it.
///
/// Over-approximating is safe and under-approximating is not. The patches are
/// read from the presentation buffer *after* the client's pixels were composed
/// into it, so a larger rectangle carries more already-correct pixels; a
/// smaller one would leave a region stale in a frame that is otherwise
/// presentable and self-consistent. `StableBackingLease`'s `RegistryMatchesStore`
/// is that property, and its second negative control is this function shrinking
/// a cover below the damage it owes.
fn coalesce_damage(mut rects: Vec<Rect>, limit: usize) -> Vec<Rect> {
    if rects.len() <= limit || limit == 0 {
        return rects;
    }
    rects.sort_by_key(|rect| (rect.y, rect.x, rect.width, rect.height));
    let mut coalesced = Vec::with_capacity(limit);
    let total = rects.len();
    for group in 0..limit {
        // Contiguous groups over the sorted list, sized so every rectangle
        // lands in exactly one and no group is empty.
        let start = group.saturating_mul(total) / limit;
        let end = group.saturating_add(1).saturating_mul(total) / limit;
        if start >= end {
            continue;
        }
        let mut bounds = rects[start];
        for rect in &rects[start.saturating_add(1)..end] {
            bounds = union_rect(bounds, *rect);
        }
        coalesced.push(bounds);
    }
    coalesced
}

fn union_rect(left: Rect, right: Rect) -> Rect {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let right_edge = left
        .x
        .saturating_add(left.width)
        .max(right.x.saturating_add(right.width));
    let bottom_edge = left
        .y
        .saturating_add(left.height)
        .max(right.y.saturating_add(right.height));
    Rect {
        x,
        y,
        width: right_edge.saturating_sub(x),
        height: bottom_edge.saturating_sub(y),
    }
}

/// The area a damage list covers, counting overlap twice.
///
/// An over-count is the safe direction here: it can only push the decision
/// toward replacing the buffer, which is always correct and merely less clever.
fn coverage_area(rects: &[Rect]) -> usize {
    rects
        .iter()
        .map(|rect| {
            usize::try_from(rect.width.max(0))
                .unwrap_or(0)
                .saturating_mul(usize::try_from(rect.height.max(0)).unwrap_or(0))
        })
        .fold(0usize, usize::saturating_add)
}

fn finish_immutable_update(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    handle: u64,
    replaced: bool,
    damage: Option<Rect>,
) -> Option<XAuthorityCpuDrawResult> {
    if !replaced {
        patch_is_representable(buffer, damage?)?;
    }
    buffer.generation = buffer.generation.checked_add(1)?;
    buffer.handle = handle;
    Some(XAuthorityCpuDrawResult {
        handle,
        size: buffer.size,
        generation: buffer.generation,
    })
}

fn union_rects(rectangles: &[Rect]) -> Option<Rect> {
    let first = *rectangles.first()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x.saturating_add(first.width);
    let mut bottom = first.y.saturating_add(first.height);
    for rect in &rectangles[1..] {
        left = left.min(rect.x);
        top = top.min(rect.y);
        right = right.max(rect.x.saturating_add(rect.width));
        bottom = bottom.max(rect.y.saturating_add(rect.height));
    }
    Some(Rect {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    })
}

pub(crate) struct XTextDraw<'a> {
    pub x: i32,
    pub baseline: i32,
    pub text: &'a [u8],
    pub image: bool,
    pub font: XFontFace,
}
