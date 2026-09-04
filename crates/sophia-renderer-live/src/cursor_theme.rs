use std::fmt;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use sophia_engine::{
    CursorAsset, CursorAssetError, CursorShape, MAX_CURSOR_EDGE, x11_core_left_ptr_cursor,
};

pub const MAX_XCURSOR_FILE_BYTES: u64 = 4 * 1024 * 1024;
const XCURSOR_IMAGE_TYPE: u32 = 0xfffd_0002;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CursorThemeError {
    ThemeNotFound,
    InheritanceTooDeep { depth: usize },
    FileTooLarge { bytes: u64 },
    Io(String),
    InvalidFile(&'static str),
    NoImages,
    InvalidAsset(CursorAssetError),
}

impl fmt::Display for CursorThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThemeNotFound => formatter.write_str("cursor theme or shape was not found"),
            Self::InheritanceTooDeep { depth } => {
                write!(
                    formatter,
                    "cursor theme inheritance depth {depth} exceeds 16"
                )
            }
            Self::FileTooLarge { bytes } => {
                write!(formatter, "cursor file size {bytes} exceeds 4 MiB")
            }
            Self::Io(error) => write!(formatter, "cursor file I/O failed: {error}"),
            Self::InvalidFile(reason) => write!(formatter, "invalid Xcursor file: {reason}"),
            Self::NoImages => formatter.write_str("Xcursor file contains no image frames"),
            Self::InvalidAsset(error) => write!(formatter, "invalid cursor asset: {error}"),
        }
    }
}

impl std::error::Error for CursorThemeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CursorResolution {
    pub asset: CursorAsset,
    pub requested_theme: String,
    pub effective_theme: String,
    pub requested_size: u32,
    pub effective_nominal_size: u32,
    pub shape: CursorShape,
    pub source: Option<PathBuf>,
    pub fallback_reason: Option<String>,
    pub ignored_animation_frames: usize,
}

pub fn resolve_cursor_theme(
    theme: &str,
    size: u32,
    shape: CursorShape,
    generation: u64,
) -> CursorResolution {
    if theme == "x11-core" && shape == CursorShape::LeftPtr {
        return CursorResolution {
            asset: x11_core_left_ptr_cursor(generation),
            requested_theme: theme.to_owned(),
            effective_theme: theme.to_owned(),
            requested_size: size,
            effective_nominal_size: 16,
            shape,
            source: None,
            fallback_reason: None,
            ignored_animation_frames: 0,
        };
    }

    match resolve_xcursor_theme(theme, size, shape, generation) {
        Ok(resolution) => resolution,
        Err(error) => CursorResolution {
            asset: x11_core_left_ptr_cursor(generation),
            requested_theme: theme.to_owned(),
            effective_theme: "x11-core".to_owned(),
            requested_size: size,
            effective_nominal_size: 16,
            shape: CursorShape::LeftPtr,
            source: None,
            fallback_reason: Some(error.to_string()),
            ignored_animation_frames: 0,
        },
    }
}

fn resolve_xcursor_theme(
    theme: &str,
    size: u32,
    shape: CursorShape,
    generation: u64,
) -> Result<CursorResolution, CursorThemeError> {
    let (path, depth) = xcursor::CursorTheme::load(theme)
        .load_icon_with_depth(shape.name())
        .ok_or(CursorThemeError::ThemeNotFound)?;
    if depth > 16 {
        return Err(CursorThemeError::InheritanceTooDeep { depth });
    }
    let bytes = read_bounded_cursor_file(&path)?;
    let decoded = decode_xcursor_asset(&bytes, size, generation)?;
    Ok(CursorResolution {
        asset: decoded.asset,
        requested_theme: theme.to_owned(),
        effective_theme: theme.to_owned(),
        requested_size: size,
        effective_nominal_size: decoded.nominal_size,
        shape,
        source: Some(path),
        fallback_reason: None,
        ignored_animation_frames: decoded.ignored_animation_frames,
    })
}

fn read_bounded_cursor_file(path: &Path) -> Result<Vec<u8>, CursorThemeError> {
    let file = fs::File::open(path).map_err(|error| CursorThemeError::Io(error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| CursorThemeError::Io(error.to_string()))?;
    if metadata.len() > MAX_XCURSOR_FILE_BYTES {
        return Err(CursorThemeError::FileTooLarge {
            bytes: metadata.len(),
        });
    }
    let mut bytes = Vec::new();
    file.take(MAX_XCURSOR_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| CursorThemeError::Io(error.to_string()))?;
    if bytes.len() as u64 > MAX_XCURSOR_FILE_BYTES {
        return Err(CursorThemeError::FileTooLarge {
            bytes: bytes.len() as u64,
        });
    }
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedXcursorAsset {
    pub asset: CursorAsset,
    pub nominal_size: u32,
    pub ignored_animation_frames: usize,
}

/// Decodes one static frame without trusting file dimensions to allocate.
///
/// Xcursor permits animation. Sophia's first cursor contract is static, so the
/// first frame at the nearest nominal size wins deterministically and the
/// ignored count is made explicit to diagnostics.
pub fn decode_xcursor_asset(
    bytes: &[u8],
    desired_size: u32,
    generation: u64,
) -> Result<DecodedXcursorAsset, CursorThemeError> {
    if bytes.len() as u64 > MAX_XCURSOR_FILE_BYTES {
        return Err(CursorThemeError::FileTooLarge {
            bytes: bytes.len() as u64,
        });
    }
    if bytes.get(0..4) != Some(b"Xcur") {
        return Err(CursorThemeError::InvalidFile("bad magic"));
    }
    let header = read_u32(bytes, 4)? as usize;
    let toc_count = read_u32(bytes, 12)? as usize;
    if header < 16 || header > bytes.len() || toc_count > 4_096 {
        return Err(CursorThemeError::InvalidFile("invalid header or TOC count"));
    }
    let toc_end = toc_count
        .checked_mul(12)
        .and_then(|length| header.checked_add(length))
        .filter(|end| *end <= bytes.len())
        .ok_or(CursorThemeError::InvalidFile("truncated TOC"))?;
    let mut images = Vec::new();
    for index in 0..toc_count {
        let entry = header + index * 12;
        if read_u32(bytes, entry)? != XCURSOR_IMAGE_TYPE {
            continue;
        }
        let nominal_size = read_u32(bytes, entry + 4)?;
        let position = read_u32(bytes, entry + 8)? as usize;
        if nominal_size == 0 || position < toc_end {
            return Err(CursorThemeError::InvalidFile("invalid image TOC entry"));
        }
        images.push((nominal_size, position));
    }
    let selected_size = images
        .iter()
        .map(|(size, _)| *size)
        .min_by_key(|size| (size.abs_diff(desired_size), *size))
        .ok_or(CursorThemeError::NoImages)?;
    let matching = images
        .iter()
        .filter(|(size, _)| *size == selected_size)
        .copied()
        .collect::<Vec<_>>();
    let (_, position) = matching[0];
    let image_header = read_u32(bytes, position)? as usize;
    if image_header < 36
        || read_u32(bytes, position + 4)? != XCURSOR_IMAGE_TYPE
        || read_u32(bytes, position + 8)? != selected_size
    {
        return Err(CursorThemeError::InvalidFile("invalid image chunk"));
    }
    let width = read_u32(bytes, position + 16)?;
    let height = read_u32(bytes, position + 20)?;
    let hotspot_x = read_u32(bytes, position + 24)?;
    let hotspot_y = read_u32(bytes, position + 28)?;
    if width == 0 || height == 0 || width > MAX_CURSOR_EDGE || height > MAX_CURSOR_EDGE {
        return Err(CursorThemeError::InvalidAsset(
            CursorAssetError::InvalidDimensions,
        ));
    }
    if hotspot_x >= width || hotspot_y >= height {
        return Err(CursorThemeError::InvalidAsset(
            CursorAssetError::InvalidHotspot,
        ));
    }
    let pixel_length = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(CursorThemeError::InvalidFile("image dimensions overflow"))?;
    let pixel_start = position
        .checked_add(image_header)
        .ok_or(CursorThemeError::InvalidFile("image offset overflow"))?;
    let pixel_end = pixel_start
        .checked_add(pixel_length)
        .filter(|end| *end <= bytes.len())
        .ok_or(CursorThemeError::InvalidFile("truncated image pixels"))?;
    let asset = CursorAsset::new(
        width,
        height,
        hotspot_x,
        hotspot_y,
        generation,
        bytes[pixel_start..pixel_end].to_vec(),
    )
    .map_err(CursorThemeError::InvalidAsset)?;
    Ok(DecodedXcursorAsset {
        asset,
        nominal_size: selected_size,
        ignored_animation_frames: matching.len().saturating_sub(1),
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, CursorThemeError> {
    let value = bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .ok_or(CursorThemeError::InvalidFile("truncated integer"))?;
    Ok(u32::from_le_bytes(value))
}
