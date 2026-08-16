use std::collections::BTreeMap;

use sophia_protocol::{Rect, Size};

use crate::{XFontFace, XGraphicsContextValues, XPoint, XResourceId};

mod raster_replay;
mod raster_variants;
mod update;

pub(crate) use raster_variants::{
    XAuthorityRasterCommand, XAuthorityRasterStore, XOwnedTextDraw, XRasterPoint,
    XRasterSatisfyOutcome, XRasterUnsupportedKind,
};
pub use raster_variants::{XPutImageSemantics, XRasterFallbackCause};
pub use update::{
    X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS, XAuthorityCpuBufferPatch, XAuthorityCpuBufferPatchBatch,
    XAuthorityCpuBufferPatchRegion, XAuthorityCpuBufferSnapshot, XAuthorityCpuBufferUpdate,
};
use update::{packed_patch, packed_patch_region};

pub const X_AUTHORITY_CPU_BUFFER_FORMAT_XRGB8888: u32 = u32::from_le_bytes(*b"XR24");
pub const X_AUTHORITY_SOFTWARE_BUFFER_MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Default)]
pub(crate) struct XSoftwareBufferStore {
    next_handle: u64,
    buffers: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
    presentations: BTreeMap<XResourceId, XAuthorityCpuBufferSnapshot>,
}

impl XSoftwareBufferStore {
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
                    bytes: vec![0; byte_len],
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
        let mut presentation_damage = Vec::with_capacity(
            damage
                .len()
                .min(X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS.saturating_add(1)),
        );
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
        if replace || presentation_damage.len() > X_AUTHORITY_CPU_PATCH_BATCH_MAX_RECTS {
            return Some(XAuthorityCpuBufferUpdate::Replace(
                presentation_buffer.clone(),
            ));
        }
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
    ) -> Option<XAuthorityCpuBufferUpdate> {
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
    ) -> Option<XAuthorityCpuBufferUpdate> {
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
    ) -> Option<XAuthorityCpuBufferUpdate> {
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
    ) -> Option<XAuthorityCpuBufferUpdate> {
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
    ) -> Option<XAuthorityCpuBufferUpdate> {
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
    ) -> Option<(XAuthorityCpuBufferUpdate, Rect)> {
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
    ) -> Option<(XAuthorityCpuBufferUpdate, Rect)> {
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
                    bytes: vec![0; byte_len],
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

fn copy_buffer_region(
    source: &XAuthorityCpuBufferSnapshot,
    destination: &mut XAuthorityCpuBufferSnapshot,
    source_rect: Rect,
    destination_x: i32,
    destination_y: i32,
) -> Option<Rect> {
    let Some((mut left, mut top, right, bottom)) = clipped_bounds(source.size, source_rect) else {
        return None;
    };
    let mut target_x = destination_x.saturating_add(i32::try_from(left).unwrap_or(i32::MAX));
    let mut target_y = destination_y.saturating_add(i32::try_from(top).unwrap_or(i32::MAX));
    if target_x < 0 {
        left =
            left.saturating_add(usize::try_from(target_x.saturating_neg()).unwrap_or(usize::MAX));
        target_x = 0;
    }
    if target_y < 0 {
        top = top.saturating_add(usize::try_from(target_y.saturating_neg()).unwrap_or(usize::MAX));
        target_y = 0;
    }
    let Ok(target_x) = usize::try_from(target_x) else {
        return None;
    };
    let Ok(target_y) = usize::try_from(target_y) else {
        return None;
    };
    let Ok(destination_width) = usize::try_from(destination.size.width) else {
        return None;
    };
    let Ok(destination_height) = usize::try_from(destination.size.height) else {
        return None;
    };
    let width = right
        .saturating_sub(left)
        .min(destination_width.saturating_sub(target_x));
    let height = bottom
        .saturating_sub(top)
        .min(destination_height.saturating_sub(target_y));
    let byte_width = width.saturating_mul(4);
    let source_stride = usize::try_from(source.stride).unwrap_or(0);
    let destination_stride = usize::try_from(destination.stride).unwrap_or(0);
    for row in 0..height {
        let source_offset = top
            .saturating_add(row)
            .saturating_mul(source_stride)
            .saturating_add(left.saturating_mul(4));
        let destination_offset = target_y
            .saturating_add(row)
            .saturating_mul(destination_stride)
            .saturating_add(target_x.saturating_mul(4));
        let Some(source_row) = source
            .bytes
            .get(source_offset..source_offset.saturating_add(byte_width))
        else {
            return None;
        };
        let Some(destination_row) = destination
            .bytes
            .get_mut(destination_offset..destination_offset.saturating_add(byte_width))
        else {
            return None;
        };
        destination_row.copy_from_slice(source_row);
    }
    (width != 0 && height != 0).then_some(Rect {
        x: i32::try_from(target_x).ok()?,
        y: i32::try_from(target_y).ok()?,
        width: i32::try_from(width).ok()?,
        height: i32::try_from(height).ok()?,
    })
}

fn finish_immutable_update(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    handle: u64,
    replaced: bool,
    damage: Option<Rect>,
) -> Option<XAuthorityCpuBufferUpdate> {
    if !replaced {
        packed_patch(buffer, damage?)?;
    }
    buffer.generation = buffer.generation.checked_add(1)?;
    buffer.handle = handle;
    Some(XAuthorityCpuBufferUpdate::Replace(buffer.clone()))
}

fn fill_rect(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    rect: Rect,
    pixel: u32,
    gc: &XGraphicsContextValues,
) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    for y in top..bottom {
        for x in left..right {
            if !pixel_in_clip(x, y, gc) {
                continue;
            }
            let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
            if let Some(target) = buffer.bytes.get_mut(offset..offset.saturating_add(4)) {
                let destination = u32::from_le_bytes(target.try_into().unwrap_or([0; 4]));
                let output = apply_raster_function(pixel, destination, gc);
                target.copy_from_slice(&output.to_le_bytes());
            }
        }
    }
}

fn set_pixel(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    x: i32,
    y: i32,
    pixel: u32,
    gc: &XGraphicsContextValues,
) {
    if x < 0 || y < 0 || x >= buffer.size.width || y >= buffer.size.height {
        return;
    }
    let Ok(x) = usize::try_from(x) else {
        return;
    };
    let Ok(y) = usize::try_from(y) else {
        return;
    };
    if !pixel_in_clip(x, y, gc) {
        return;
    }
    let stride = usize::try_from(buffer.stride).unwrap_or(0);
    let offset = y.saturating_mul(stride).saturating_add(x.saturating_mul(4));
    if let Some(target) = buffer.bytes.get_mut(offset..offset.saturating_add(4)) {
        let destination = u32::from_le_bytes(target.try_into().unwrap_or([0; 4]));
        target.copy_from_slice(&apply_raster_function(pixel, destination, gc).to_le_bytes());
    }
}

fn draw_line(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    from: XPoint,
    to: XPoint,
    width: i32,
    gc: &XGraphicsContextValues,
) {
    let mut x = i32::from(from.x);
    let mut y = i32::from(from.y);
    let target_x = i32::from(to.x);
    let target_y = i32::from(to.y);
    let dx = (target_x - x).abs();
    let sx = if x < target_x { 1 } else { -1 };
    let dy = -(target_y - y).abs();
    let sy = if y < target_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        let offset = width / 2;
        fill_rect(
            buffer,
            Rect {
                x: x.saturating_sub(offset),
                y: y.saturating_sub(offset),
                width,
                height: width,
            },
            gc.foreground,
            gc,
        );
        if x == target_x && y == target_y {
            break;
        }
        let doubled = error.saturating_mul(2);
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

fn draw_rectangle_outline(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    rectangle: Rect,
    line_width: i32,
    gc: &XGraphicsContextValues,
) {
    let half = line_width / 2;
    let outer_left = rectangle.x.saturating_sub(half);
    let outer_top = rectangle.y.saturating_sub(half);
    let outer_right = rectangle
        .x
        .saturating_add(rectangle.width)
        .saturating_sub(half)
        .saturating_add(line_width);
    let outer_bottom = rectangle
        .y
        .saturating_add(rectangle.height)
        .saturating_sub(half)
        .saturating_add(line_width);
    let inner_left = outer_left.saturating_add(line_width);
    let inner_top = outer_top.saturating_add(line_width);
    let inner_right = outer_right.saturating_sub(line_width);
    let inner_bottom = outer_bottom.saturating_sub(line_width);
    let outer = Rect {
        x: outer_left,
        y: outer_top,
        width: outer_right.saturating_sub(outer_left),
        height: outer_bottom.saturating_sub(outer_top),
    };
    if inner_left >= inner_right || inner_top >= inner_bottom {
        fill_rect(buffer, outer, gc.foreground, gc);
        return;
    }

    // The four bands do not overlap. That keeps every pixel to one raster operation,
    // and fill_rect clips each band before walking it.
    for band in [
        Rect {
            x: outer_left,
            y: outer_top,
            width: outer.width,
            height: inner_top.saturating_sub(outer_top),
        },
        Rect {
            x: outer_left,
            y: inner_bottom,
            width: outer.width,
            height: outer_bottom.saturating_sub(inner_bottom),
        },
        Rect {
            x: outer_left,
            y: inner_top,
            width: inner_left.saturating_sub(outer_left),
            height: inner_bottom.saturating_sub(inner_top),
        },
        Rect {
            x: inner_right,
            y: inner_top,
            width: outer_right.saturating_sub(inner_right),
            height: inner_bottom.saturating_sub(inner_top),
        },
    ] {
        fill_rect(buffer, band, gc.foreground, gc);
    }
}

fn rectangle_outline_bounds(rectangles: &[Rect], line_width: u16) -> Option<Rect> {
    let first = *rectangles.first()?;
    let mut left = first.x;
    let mut top = first.y;
    let mut right = first.x.saturating_add(first.width);
    let mut bottom = first.y.saturating_add(first.height);
    for rectangle in &rectangles[1..] {
        left = left.min(rectangle.x);
        top = top.min(rectangle.y);
        right = right.max(rectangle.x.saturating_add(rectangle.width));
        bottom = bottom.max(rectangle.y.saturating_add(rectangle.height));
    }
    let width = i32::from(line_width.max(1));
    let half = width / 2;
    Some(Rect {
        x: left.saturating_sub(half),
        y: top.saturating_sub(half),
        width: right.saturating_sub(left).saturating_add(width),
        height: bottom.saturating_sub(top).saturating_add(width),
    })
}

fn point_bounds(points: &[XPoint], line_width: u16) -> Option<Rect> {
    let first = *points.first()?;
    let mut left = i32::from(first.x);
    let mut top = i32::from(first.y);
    let mut right = left;
    let mut bottom = top;
    for point in &points[1..] {
        let x = i32::from(point.x);
        let y = i32::from(point.y);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x);
        bottom = bottom.max(y);
    }
    let width = i32::from(line_width.max(1));
    let half = width / 2;
    Some(Rect {
        x: left.saturating_sub(half),
        y: top.saturating_sub(half),
        width: right.saturating_sub(left).saturating_add(width),
        height: bottom.saturating_sub(top).saturating_add(width),
    })
}

fn copy_xrgb8888(buffer: &mut XAuthorityCpuBufferSnapshot, rect: Rect, data: &[u8]) {
    let Some((left, top, right, bottom)) = clipped_bounds(buffer.size, rect) else {
        return;
    };
    let source_width = usize::try_from(rect.width.max(0)).unwrap_or(0);
    let source_height = usize::try_from(rect.height.max(0)).unwrap_or(0);
    let Some(source_stride) = source_width.checked_mul(4) else {
        return;
    };
    if data.len() < source_stride.saturating_mul(source_height) {
        return;
    }
    let target_stride = usize::try_from(buffer.stride).unwrap_or(0);
    for y in top..bottom {
        let source_y = y.saturating_sub(usize::try_from(rect.y.max(0)).unwrap_or(0));
        let source_x = left.saturating_sub(usize::try_from(rect.x.max(0)).unwrap_or(0));
        let width = right.saturating_sub(left);
        let source_offset = source_y
            .saturating_mul(source_stride)
            .saturating_add(source_x.saturating_mul(4));
        let target_offset = y
            .saturating_mul(target_stride)
            .saturating_add(left.saturating_mul(4));
        let byte_len = width.saturating_mul(4);
        let Some(source) = data.get(source_offset..source_offset.saturating_add(byte_len)) else {
            continue;
        };
        if let Some(target) = buffer
            .bytes
            .get_mut(target_offset..target_offset.saturating_add(byte_len))
        {
            target.copy_from_slice(source);
        }
    }
}

fn draw_fixed_glyph(
    buffer: &mut XAuthorityCpuBufferSnapshot,
    cell_x: i32,
    cell_y: i32,
    byte: u8,
    pixel: u32,
    font: XFontFace,
    gc: &XGraphicsContextValues,
) {
    let rows = font.glyph_rows(byte);
    for (row, bits) in rows.into_iter().enumerate() {
        for column in 0..6 {
            if bits & (1 << (5 - column)) == 0 {
                continue;
            }
            fill_rect(
                buffer,
                Rect {
                    x: cell_x.saturating_add(column),
                    y: cell_y.saturating_add(i32::try_from(row).unwrap_or(0)),
                    width: 1,
                    height: 1,
                },
                pixel,
                gc,
            );
        }
    }
}

fn clipped_bounds(size: Size, rect: Rect) -> Option<(usize, usize, usize, usize)> {
    if size.width <= 0 || size.height <= 0 || rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let left = rect.x.max(0).min(size.width);
    let top = rect.y.max(0).min(size.height);
    let right = rect.x.saturating_add(rect.width).max(0).min(size.width);
    let bottom = rect.y.saturating_add(rect.height).max(0).min(size.height);
    if right <= left || bottom <= top {
        return None;
    }
    Some((
        usize::try_from(left).ok()?,
        usize::try_from(top).ok()?,
        usize::try_from(right).ok()?,
        usize::try_from(bottom).ok()?,
    ))
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

fn pixel_in_clip(x: usize, y: usize, gc: &XGraphicsContextValues) -> bool {
    if gc.clip_rectangles.is_empty() {
        return true;
    }
    let x = i32::try_from(x).unwrap_or(i32::MAX);
    let y = i32::try_from(y).unwrap_or(i32::MAX);
    gc.clip_rectangles.iter().any(|rect| {
        let left = rect.x.saturating_add(i32::from(gc.clip_x_origin));
        let top = rect.y.saturating_add(i32::from(gc.clip_y_origin));
        x >= left
            && y >= top
            && x < left.saturating_add(rect.width)
            && y < top.saturating_add(rect.height)
    })
}

fn apply_raster_function(source: u32, destination: u32, gc: &XGraphicsContextValues) -> u32 {
    let source = source & 0x00ff_ffff;
    let destination = destination & 0x00ff_ffff;
    let result = match gc.function {
        0 => 0,
        1 => source & destination,
        2 => source & !destination,
        3 => source,
        4 => !source & destination,
        5 => destination,
        6 => source ^ destination,
        7 => source | destination,
        8 => !(source | destination),
        9 => !(source ^ destination),
        10 => !destination,
        11 => source | !destination,
        12 => !source,
        13 => !source | destination,
        14 => !(source & destination),
        15 => u32::MAX,
        _ => source,
    } & 0x00ff_ffff;
    let mask = gc.plane_mask & 0x00ff_ffff;
    ((result & mask) | (destination & !mask)) & 0x00ff_ffff
}

pub(crate) struct XTextDraw<'a> {
    pub x: i32,
    pub baseline: i32,
    pub text: &'a [u8],
    pub image: bool,
    pub font: XFontFace,
}
