use crate::prelude::*;

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
