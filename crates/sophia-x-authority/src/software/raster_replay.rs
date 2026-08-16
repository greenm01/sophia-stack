//! Deterministic replay of the authority-private semantic journal into a
//! derived density store.
//!
//! Every projection here is exact integer arithmetic over the command's own
//! 1x coordinates. Replay never samples the canonical drawable, so a derived
//! store is independently rendered content rather than a resampled copy.

use sophia_protocol::{Rect, SurfaceRasterClass};

use crate::{X_GX_COPY, XFontFace, XGraphicsContextValues};

use super::raster_variants::{XAuthorityRasterCommand, XOwnedImagePixels, XOwnedTextDraw};
use super::{XAuthorityCpuBufferSnapshot, draw_line, draw_rectangle_outline, fill_rect, set_pixel};

pub(super) fn floor_edge(value: i32, density: u32) -> i32 {
    let scaled = i64::from(value).saturating_mul(i64::from(density));
    let quotient = scaled.div_euclid(1_000);
    i32::try_from(quotient).unwrap_or(if quotient < 0 { i32::MIN } else { i32::MAX })
}

pub(super) fn ceil_edge(value: i32, density: u32) -> i32 {
    let scaled = i64::from(value).saturating_mul(i64::from(density));
    let quotient = scaled.saturating_add(999).div_euclid(1_000);
    i32::try_from(quotient).unwrap_or(if quotient < 0 { i32::MIN } else { i32::MAX })
}

pub(super) fn project_rect(rect: Rect, density: u32) -> Rect {
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

pub(super) fn apply_command(
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
        XAuthorityRasterCommand::PutImage { image, gc } => {
            blit_projected_image(snapshot, density, image, gc);
        }
        XAuthorityRasterCommand::Unsupported(_) => {}
    }
}

/// Projects retained client pixels into one derived density store.
///
/// The retained 1x pixels are the source of truth, not the canonical store, so
/// replay stays correctly ordered against later text and copy commands. Each
/// destination pixel is an exact rational area average of the source pixels it
/// covers, matching the glyph coverage rule. At 1x the projection degenerates
/// to full coverage of one source pixel, so the derived store reproduces the
/// canonical drawable bit for bit.
fn blit_projected_image(
    snapshot: &mut XAuthorityCpuBufferSnapshot,
    density: u32,
    image: &XOwnedImagePixels,
    gc: &XGraphicsContextValues,
) {
    let width = usize::try_from(image.rect.width.max(0)).unwrap_or(0);
    let height = usize::try_from(image.rect.height.max(0)).unwrap_or(0);
    if density == 0 || width == 0 || height == 0 {
        return;
    }
    let source_stride = width.saturating_mul(4);
    let gc = projected_gc(gc, density);
    let bounds = project_rect(image.rect, density);
    for y in bounds.y..bounds.y.saturating_add(bounds.height) {
        let (row_start, row_end) = source_span(y, density, image.rect.y, image.rect.height);
        for x in bounds.x..bounds.x.saturating_add(bounds.width) {
            let (column_start, column_end) =
                source_span(x, density, image.rect.x, image.rect.width);
            let mut area = 0_i64;
            let mut channels = [0_i64; 3];
            for sy in row_start..row_end {
                let overlap_y = overlap_scaled(y, sy, density);
                if overlap_y == 0 {
                    continue;
                }
                for sx in column_start..column_end {
                    let overlap_x = overlap_scaled(x, sx, density);
                    if overlap_x == 0 {
                        continue;
                    }
                    let Some(pixel) = source_pixel(image, source_stride, sx, sy) else {
                        continue;
                    };
                    let weight = overlap_x.saturating_mul(overlap_y);
                    area = area.saturating_add(weight);
                    for (index, channel) in channels.iter_mut().enumerate() {
                        // Accumulate squared components so the average is a
                        // mix of light rather than of gamma-encoded bytes;
                        // see `blend_copy_pixel` for why the plain mean
                        // darkens every partially covered pixel.
                        let component = i64::from((pixel >> (8 * index as u32)) & 0xff);
                        *channel = channel.saturating_add(
                            component.saturating_mul(component).saturating_mul(weight),
                        );
                    }
                }
            }
            if area <= 0 {
                continue;
            }
            let component = |channel: i64| -> u32 {
                let mixed = channel.saturating_add(area / 2) / area;
                u32::try_from(mixed.max(0).isqrt())
                    .unwrap_or(0xff)
                    .min(0xff)
            };
            let pixel =
                component(channels[2]) << 16 | component(channels[1]) << 8 | component(channels[0]);
            let coverage =
                u8::try_from(area.saturating_mul(255).saturating_add(500_000) / 1_000_000)
                    .unwrap_or(u8::MAX);
            if coverage == 0 {
                continue;
            }
            if gc.function == X_GX_COPY {
                blend_copy_pixel(snapshot, x, y, pixel, coverage, &gc);
            } else if coverage >= 128 {
                set_pixel(snapshot, x, y, pixel, &gc);
            }
        }
    }
}

/// Source index range, clamped to the image, that can overlap one destination
/// pixel. Bounding the inner loop keeps replay linear in destination pixels
/// instead of quadratic in the image.
fn source_span(destination: i32, density: u32, origin: i32, extent: i32) -> (i32, i32) {
    let density = i64::from(density).max(1);
    let left = i64::from(destination).saturating_mul(1_000);
    let start = left.div_euclid(density);
    let end = left
        .saturating_add(1_000)
        .saturating_add(density.saturating_sub(1))
        .div_euclid(density);
    let low = i64::from(origin);
    let high = low.saturating_add(i64::from(extent.max(0)));
    let clamp = |value: i64| i32::try_from(value.clamp(low, high)).unwrap_or(origin);
    (clamp(start), clamp(end))
}

fn source_pixel(image: &XOwnedImagePixels, stride: usize, sx: i32, sy: i32) -> Option<u32> {
    let column = usize::try_from(sx.checked_sub(image.rect.x)?).ok()?;
    let row = usize::try_from(sy.checked_sub(image.rect.y)?).ok()?;
    let offset = row
        .checked_mul(stride)?
        .checked_add(column.checked_mul(4)?)?;
    let bytes = image.pixels.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
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

/// Exact rational overlap, in millis, between one destination pixel and one
/// source pixel projected at `density`.
pub(super) fn overlap_scaled(destination: i32, source: i32, density: u32) -> i64 {
    let destination_left = i64::from(destination).saturating_mul(1_000);
    let destination_right = destination_left.saturating_add(1_000);
    let source_left = i64::from(source).saturating_mul(i64::from(density));
    let source_right = source_left.saturating_add(i64::from(density));
    destination_right
        .min(source_right)
        .saturating_sub(destination_left.max(source_left))
        .max(0)
}

pub(super) fn blend_copy_pixel(
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
    // Partial coverage is a light-intensity mix, so it has to be weighted in
    // linear space. Channel bytes are gamma-encoded, and averaging them
    // directly makes every antialiased edge far darker than its coverage:
    // half-covered white on black lands near 128, roughly a fifth of the
    // intended luminance instead of half. Strokes thinner than a pixel are
    // then uniformly under-weighted, which reads as out of focus rather than
    // merely soft.
    //
    // Squaring approximates the encoding closely enough to fix that while
    // staying exact integer arithmetic, so replay remains bit-reproducible;
    // a real transfer function needs a power the platform is free to round
    // differently. Full and zero coverage still map to the endpoints exactly,
    // so canonical-density text stays bit-identical to the 1x drawable.
    let blend = |shift: u32| -> u32 {
        let source = (source >> shift) & 0xff;
        let destination = (destination >> shift) & 0xff;
        let mixed = source
            .saturating_mul(source)
            .saturating_mul(alpha)
            .saturating_add(
                destination
                    .saturating_mul(destination)
                    .saturating_mul(255 - alpha),
            )
            .saturating_add(127)
            / 255;
        mixed.isqrt().min(0xff)
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
