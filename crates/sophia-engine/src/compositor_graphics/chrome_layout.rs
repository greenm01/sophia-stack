use crate::prelude::*;
use sophia_protocol::SurfaceConstraints;

use super::SurfaceChromeStyle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChromeLayoutError {
    InvalidClearance,
    AllocationTooSmall,
    CoordinateOverflow,
}

impl fmt::Display for ChromeLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ChromeLayoutError {}

/// Converts a WM-owned outer allocation transaction into client-content
/// geometry using one stable Engine-owned chrome clearance.
pub fn apply_surface_chrome_clearance(
    transaction: &LayoutTransaction,
    style: SurfaceChromeStyle,
) -> Result<LayoutTransaction, ChromeLayoutError> {
    let clearance = style.clearance();
    if clearance < 0 {
        return Err(ChromeLayoutError::InvalidClearance);
    }
    let mut content = transaction.clone();
    for request in &mut content.requested_sizes {
        request.size = inset_size(request.size, clearance)?;
    }
    for placement in &mut content.render_positions {
        placement.geometry = inset_rect(placement.geometry, clearance)?;
    }
    Ok(content)
}

/// Converts client-content constraints into the outer allocation constraints
/// consumed by a blind WM. This is the inverse size operation of
/// `apply_surface_chrome_clearance`.
pub fn outer_surface_constraints(
    constraints: SurfaceConstraints,
    style: SurfaceChromeStyle,
) -> Result<SurfaceConstraints, ChromeLayoutError> {
    Ok(SurfaceConstraints {
        min_size: constraints
            .min_size
            .map(|size| outset_size(size, style.clearance()))
            .transpose()?,
        max_size: constraints
            .max_size
            .map(|size| outset_size(size, style.clearance()))
            .transpose()?,
    })
}

/// Converts committed client-content geometry back to the outer allocation
/// geometry understood by a blind WM.
pub fn outer_surface_geometry(
    geometry: Rect,
    style: SurfaceChromeStyle,
) -> Result<Rect, ChromeLayoutError> {
    let clearance = style.clearance();
    let size = outset_size(
        Size {
            width: geometry.width,
            height: geometry.height,
        },
        clearance,
    )?;
    Ok(Rect {
        x: geometry
            .x
            .checked_sub(clearance)
            .ok_or(ChromeLayoutError::CoordinateOverflow)?,
        y: geometry
            .y
            .checked_sub(clearance)
            .ok_or(ChromeLayoutError::CoordinateOverflow)?,
        width: size.width,
        height: size.height,
    })
}

fn inset_size(size: Size, clearance: i32) -> Result<Size, ChromeLayoutError> {
    let doubled = clearance
        .checked_mul(2)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    let width = size
        .width
        .checked_sub(doubled)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    let height = size
        .height
        .checked_sub(doubled)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    if width <= 0 || height <= 0 {
        return Err(ChromeLayoutError::AllocationTooSmall);
    }
    Ok(Size { width, height })
}

fn outset_size(size: Size, clearance: i32) -> Result<Size, ChromeLayoutError> {
    if clearance < 0 {
        return Err(ChromeLayoutError::InvalidClearance);
    }
    if size.width <= 0 || size.height <= 0 {
        return Err(ChromeLayoutError::AllocationTooSmall);
    }
    let doubled = clearance
        .checked_mul(2)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    let width = size
        .width
        .checked_add(doubled)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    let height = size
        .height
        .checked_add(doubled)
        .ok_or(ChromeLayoutError::CoordinateOverflow)?;
    Ok(Size { width, height })
}

fn inset_rect(rect: Rect, clearance: i32) -> Result<Rect, ChromeLayoutError> {
    let size = inset_size(
        Size {
            width: rect.width,
            height: rect.height,
        },
        clearance,
    )?;
    Ok(Rect {
        x: rect
            .x
            .checked_add(clearance)
            .ok_or(ChromeLayoutError::CoordinateOverflow)?,
        y: rect
            .y
            .checked_add(clearance)
            .ok_or(ChromeLayoutError::CoordinateOverflow)?,
        width: size.width,
        height: size.height,
    })
}
