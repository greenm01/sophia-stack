use std::collections::BTreeMap;

use sophia_protocol::{
    AxisSpan, NamespaceId, OutputEdge, OutputReservation, Rect, Size, SurfaceConstraints,
};

use crate::{
    X_ATOM_CARDINAL, X_ATOM_NAME_NET_WM_STRUT, X_ATOM_NAME_NET_WM_STRUT_PARTIAL, XAtom, XAtomTable,
    XByteOrder, XResourceId, is_metadata_candidate_name,
};

pub const X_PROPERTY_MAX_VALUE_BYTES: usize = 256 * 1024;
pub const X_PROPERTY_MAX_TABLE_BYTES: usize = 4 * 1024 * 1024;
pub const X_PROPERTY_ANY_TYPE: XAtom = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XPropertyMode {
    Replace,
    Prepend,
    Append,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyChange {
    pub mode: XPropertyMode,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub format: u8,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyRead {
    pub delete: bool,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub long_offset: u32,
    pub long_length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyReadReply {
    pub property_type: XAtom,
    pub format: u8,
    pub bytes_after: u32,
    pub item_count: u32,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyReadOutcome {
    pub reply: XPropertyReadReply,
    pub deleted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XPropertyRecord {
    pub namespace: NamespaceId,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_type: XAtom,
    pub format: u8,
    pub bytes: Vec<u8>,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XMetadataPropertyCandidate {
    pub namespace: NamespaceId,
    pub window: XResourceId,
    pub property: XAtom,
    pub property_name: String,
    pub property_type: XAtom,
    pub property_type_name: Option<String>,
    pub format: u8,
    pub byte_len: usize,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XPropertyError {
    InvalidNamespace,
    InvalidWindow,
    InvalidFormat(u8),
    ValueTooLarge { len: usize, max: usize },
    TableTooLarge { len: usize, max: usize },
    TypeMismatch,
    InvalidOffset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XOutputReservationDecodeError {
    InvalidRoot,
    InvalidType,
    InvalidFormat,
    InvalidLength,
    ValueOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XSizeHintsDecodeError {
    InvalidType,
    InvalidFormat,
    InvalidLength,
    InvalidExtent,
    InvalidBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XTransientForDecodeError {
    InvalidType,
    InvalidFormat,
    InvalidLength,
    InvalidWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XWindowTypeDecodeError {
    InvalidType,
    InvalidFormat,
    InvalidLength,
}

/// Reduces EWMH functional window types to the presentation distinction the
/// Engine needs. Unknown extension atoms are skipped, as required by EWMH;
/// a missing recognized type falls back to a normal policy-managed toplevel.
pub fn decode_x_window_type_client_positioned(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    byte_order: XByteOrder,
) -> Option<Result<bool, XWindowTypeDecodeError>> {
    if atoms.name(record.property) != Some("_NET_WM_WINDOW_TYPE") {
        return None;
    }
    if atoms.name(record.property_type) != Some("ATOM") {
        return Some(Err(XWindowTypeDecodeError::InvalidType));
    }
    if record.format != 32 {
        return Some(Err(XWindowTypeDecodeError::InvalidFormat));
    }
    if record.bytes.is_empty() || record.bytes.len() % 4 != 0 {
        return Some(Err(XWindowTypeDecodeError::InvalidLength));
    }

    let client_positioned = record.bytes.chunks_exact(4).find_map(|bytes| {
        let atom = byte_order.u32(bytes);
        match atoms.name(atom) {
            Some("_NET_WM_WINDOW_TYPE_NORMAL") => Some(false),
            Some("_NET_WM_WINDOW_TYPE_DESKTOP")
            | Some("_NET_WM_WINDOW_TYPE_DOCK")
            | Some("_NET_WM_WINDOW_TYPE_TOOLBAR")
            | Some("_NET_WM_WINDOW_TYPE_MENU")
            | Some("_NET_WM_WINDOW_TYPE_UTILITY")
            | Some("_NET_WM_WINDOW_TYPE_SPLASH")
            | Some("_NET_WM_WINDOW_TYPE_DIALOG")
            | Some("_NET_WM_WINDOW_TYPE_DROPDOWN_MENU")
            | Some("_NET_WM_WINDOW_TYPE_POPUP_MENU")
            | Some("_NET_WM_WINDOW_TYPE_TOOLTIP")
            | Some("_NET_WM_WINDOW_TYPE_NOTIFICATION")
            | Some("_NET_WM_WINDOW_TYPE_COMBO")
            | Some("_NET_WM_WINDOW_TYPE_DND") => Some(true),
            _ => None,
        }
    });
    Some(Ok(client_positioned.unwrap_or(false)))
}

pub fn decode_x_transient_for(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    byte_order: XByteOrder,
) -> Option<Result<XResourceId, XTransientForDecodeError>> {
    if atoms.name(record.property) != Some("WM_TRANSIENT_FOR") {
        return None;
    }
    if atoms.name(record.property_type) != Some("WINDOW") {
        return Some(Err(XTransientForDecodeError::InvalidType));
    }
    if record.format != 32 {
        return Some(Err(XTransientForDecodeError::InvalidFormat));
    }
    if record.bytes.len() != 4 {
        return Some(Err(XTransientForDecodeError::InvalidLength));
    }
    let raw = u64::from(byte_order.u32(&record.bytes));
    let owner = XResourceId::new(raw, 1);
    if !owner.is_valid() {
        return Some(Err(XTransientForDecodeError::InvalidWindow));
    }
    Some(Ok(owner))
}

pub fn decode_x_size_hints(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    byte_order: XByteOrder,
) -> Option<Result<SurfaceConstraints, XSizeHintsDecodeError>> {
    if atoms.name(record.property) != Some("WM_NORMAL_HINTS") {
        return None;
    }
    if atoms.name(record.property_type) != Some("WM_SIZE_HINTS") {
        return Some(Err(XSizeHintsDecodeError::InvalidType));
    }
    if record.format != 32 {
        return Some(Err(XSizeHintsDecodeError::InvalidFormat));
    }
    if record.bytes.len() < 9 * 4 {
        return Some(Err(XSizeHintsDecodeError::InvalidLength));
    }
    let value = |index: usize| byte_order.u32(&record.bytes[index * 4..index * 4 + 4]) as i32;
    let flags = value(0) as u32;
    let extent = |width_index: usize, height_index: usize| {
        let size = Size {
            width: value(width_index),
            height: value(height_index),
        };
        (size.width > 0 && size.height > 0)
            .then_some(size)
            .ok_or(XSizeHintsDecodeError::InvalidExtent)
    };
    const P_MIN_SIZE: u32 = 1 << 4;
    const P_MAX_SIZE: u32 = 1 << 5;
    let min_size = if flags & P_MIN_SIZE != 0 {
        match extent(5, 6) {
            Ok(size) => Some(size),
            Err(error) => return Some(Err(error)),
        }
    } else {
        None
    };
    let max_size = if flags & P_MAX_SIZE != 0 {
        match extent(7, 8) {
            Ok(size) => Some(size),
            Err(error) => return Some(Err(error)),
        }
    } else {
        None
    };
    if matches!((min_size, max_size), (Some(minimum), Some(maximum))
        if minimum.width > maximum.width || minimum.height > maximum.height)
    {
        return Some(Err(XSizeHintsDecodeError::InvalidBounds));
    }
    Some(Ok(SurfaceConstraints { min_size, max_size }))
}

pub fn decode_x_output_reservations(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    byte_order: XByteOrder,
    root: Rect,
) -> Option<Result<Vec<OutputReservation>, XOutputReservationDecodeError>> {
    let property_name = atoms.name(record.property)?;
    match property_name {
        X_ATOM_NAME_NET_WM_STRUT_PARTIAL => {
            Some(decode_partial_output_reservations(record, byte_order, root))
        }
        X_ATOM_NAME_NET_WM_STRUT => {
            Some(decode_legacy_output_reservations(record, byte_order, root))
        }
        _ => None,
    }
}

pub fn x_output_reservations_for_window(
    properties: &XPropertyTable,
    atoms: &XAtomTable,
    namespace: NamespaceId,
    window: XResourceId,
    byte_order: XByteOrder,
    root: Rect,
) -> Vec<OutputReservation> {
    let partial = atoms
        .atom(X_ATOM_NAME_NET_WM_STRUT_PARTIAL)
        .and_then(|property| properties.get(namespace, window, property))
        .and_then(|record| decode_x_output_reservations(record, atoms, byte_order, root))
        .and_then(Result::ok);
    if let Some(reservations) = partial {
        return reservations;
    }

    atoms
        .atom(X_ATOM_NAME_NET_WM_STRUT)
        .and_then(|property| properties.get(namespace, window, property))
        .and_then(|record| decode_x_output_reservations(record, atoms, byte_order, root))
        .and_then(Result::ok)
        .unwrap_or_default()
}

pub fn metadata_property_candidate(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
) -> Option<XMetadataPropertyCandidate> {
    let property_name = atoms.name(record.property)?;
    if !is_metadata_candidate_name(property_name) {
        return None;
    }
    Some(XMetadataPropertyCandidate {
        namespace: record.namespace,
        window: record.window,
        property: record.property,
        property_name: property_name.to_owned(),
        property_type: record.property_type,
        property_type_name: atoms
            .name(record.property_type)
            .map(std::borrow::ToOwned::to_owned),
        format: record.format,
        byte_len: record.bytes.len(),
        generation: record.generation,
    })
}

fn decode_partial_output_reservations(
    record: &XPropertyRecord,
    byte_order: XByteOrder,
    root: Rect,
) -> Result<Vec<OutputReservation>, XOutputReservationDecodeError> {
    const PARTIAL_CARDINAL_COUNT: usize = 12;
    let values = decode_cardinals(record, byte_order, PARTIAL_CARDINAL_COUNT)?;
    let root_horizontal = root_horizontal_span(root)?;
    let root_vertical = root_vertical_span(root)?;
    let mut reservations = Vec::with_capacity(4);
    push_output_reservation(
        &mut reservations,
        OutputEdge::Left,
        values[0],
        values[4],
        values[5],
        root.width,
        root_vertical,
    )?;
    push_output_reservation(
        &mut reservations,
        OutputEdge::Right,
        values[1],
        values[6],
        values[7],
        root.width,
        root_vertical,
    )?;
    push_output_reservation(
        &mut reservations,
        OutputEdge::Top,
        values[2],
        values[8],
        values[9],
        root.height,
        root_horizontal,
    )?;
    push_output_reservation(
        &mut reservations,
        OutputEdge::Bottom,
        values[3],
        values[10],
        values[11],
        root.height,
        root_horizontal,
    )?;
    Ok(reservations)
}

fn decode_legacy_output_reservations(
    record: &XPropertyRecord,
    byte_order: XByteOrder,
    root: Rect,
) -> Result<Vec<OutputReservation>, XOutputReservationDecodeError> {
    const LEGACY_CARDINAL_COUNT: usize = 4;
    let values = decode_cardinals(record, byte_order, LEGACY_CARDINAL_COUNT)?;
    let horizontal = root_horizontal_span(root)?;
    let vertical = root_vertical_span(root)?;
    let mut reservations = Vec::with_capacity(4);
    push_legacy_output_reservation(
        &mut reservations,
        OutputEdge::Left,
        values[0],
        root.width,
        vertical,
    )?;
    push_legacy_output_reservation(
        &mut reservations,
        OutputEdge::Right,
        values[1],
        root.width,
        vertical,
    )?;
    push_legacy_output_reservation(
        &mut reservations,
        OutputEdge::Top,
        values[2],
        root.height,
        horizontal,
    )?;
    push_legacy_output_reservation(
        &mut reservations,
        OutputEdge::Bottom,
        values[3],
        root.height,
        horizontal,
    )?;
    Ok(reservations)
}

fn decode_cardinals(
    record: &XPropertyRecord,
    byte_order: XByteOrder,
    count: usize,
) -> Result<Vec<u32>, XOutputReservationDecodeError> {
    if record.property_type != X_ATOM_CARDINAL {
        return Err(XOutputReservationDecodeError::InvalidType);
    }
    if record.format != 32 {
        return Err(XOutputReservationDecodeError::InvalidFormat);
    }
    if record.bytes.len() != count.saturating_mul(4) {
        return Err(XOutputReservationDecodeError::InvalidLength);
    }
    Ok(record
        .bytes
        .chunks_exact(4)
        .map(|bytes| byte_order.u32(bytes))
        .collect())
}

fn push_output_reservation(
    reservations: &mut Vec<OutputReservation>,
    edge: OutputEdge,
    depth: u32,
    start: u32,
    inclusive_end: u32,
    maximum_depth: i32,
    root_span: AxisSpan,
) -> Result<(), XOutputReservationDecodeError> {
    if depth == 0 {
        return Ok(());
    }
    let depth = i32::try_from(depth).map_err(|_| XOutputReservationDecodeError::ValueOutOfRange)?;
    let start = i32::try_from(start).map_err(|_| XOutputReservationDecodeError::ValueOutOfRange)?;
    let end = inclusive_end
        .checked_add(1)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(XOutputReservationDecodeError::ValueOutOfRange)?;
    let span = AxisSpan { start, end };
    if depth > maximum_depth
        || span.is_empty()
        || span.start < root_span.start
        || span.end > root_span.end
    {
        return Err(XOutputReservationDecodeError::ValueOutOfRange);
    }
    reservations.push(OutputReservation { edge, depth, span });
    Ok(())
}

fn push_legacy_output_reservation(
    reservations: &mut Vec<OutputReservation>,
    edge: OutputEdge,
    depth: u32,
    maximum_depth: i32,
    span: AxisSpan,
) -> Result<(), XOutputReservationDecodeError> {
    if depth == 0 {
        return Ok(());
    }
    let depth = i32::try_from(depth).map_err(|_| XOutputReservationDecodeError::ValueOutOfRange)?;
    if depth > maximum_depth {
        return Err(XOutputReservationDecodeError::ValueOutOfRange);
    }
    reservations.push(OutputReservation { edge, depth, span });
    Ok(())
}

fn root_horizontal_span(root: Rect) -> Result<AxisSpan, XOutputReservationDecodeError> {
    if root.is_empty() {
        return Err(XOutputReservationDecodeError::InvalidRoot);
    }
    Ok(AxisSpan {
        start: root.x,
        end: root
            .x
            .checked_add(root.width)
            .ok_or(XOutputReservationDecodeError::InvalidRoot)?,
    })
}

fn root_vertical_span(root: Rect) -> Result<AxisSpan, XOutputReservationDecodeError> {
    if root.is_empty() {
        return Err(XOutputReservationDecodeError::InvalidRoot);
    }
    Ok(AxisSpan {
        start: root.y,
        end: root
            .y
            .checked_add(root.height)
            .ok_or(XOutputReservationDecodeError::InvalidRoot)?,
    })
}

impl core::fmt::Display for XPropertyError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for XPropertyError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct XPropertyTable {
    records: BTreeMap<(NamespaceId, XResourceId, XAtom), XPropertyRecord>,
}

impl XPropertyTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_change(
        &mut self,
        namespace: NamespaceId,
        change: XPropertyChange,
    ) -> Result<XPropertyRecord, XPropertyError> {
        if !namespace.is_valid() {
            return Err(XPropertyError::InvalidNamespace);
        }
        if !change.window.is_valid() {
            return Err(XPropertyError::InvalidWindow);
        }
        validate_property_format(change.format)?;
        if change.bytes.len() > X_PROPERTY_MAX_VALUE_BYTES {
            return Err(XPropertyError::ValueTooLarge {
                len: change.bytes.len(),
                max: X_PROPERTY_MAX_VALUE_BYTES,
            });
        }

        let key = (namespace, change.window, change.property);
        let previous = self.records.get(&key);
        let previous_len = previous.map_or(0, |record| record.bytes.len());
        let generation = previous
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);
        let bytes = match (change.mode, previous) {
            (XPropertyMode::Replace, _) | (_, None) => change.bytes,
            (XPropertyMode::Append, Some(record)) => {
                ensure_same_property_shape(record, &change)?;
                joined_bytes(&record.bytes, &change.bytes)?
            }
            (XPropertyMode::Prepend, Some(record)) => {
                ensure_same_property_shape(record, &change)?;
                joined_bytes(&change.bytes, &record.bytes)?
            }
        };
        let table_len = self
            .records
            .values()
            .try_fold(0usize, |total, record| {
                total.checked_add(record.bytes.len())
            })
            .and_then(|total| total.checked_sub(previous_len))
            .and_then(|total| total.checked_add(bytes.len()))
            .ok_or(XPropertyError::TableTooLarge {
                len: usize::MAX,
                max: X_PROPERTY_MAX_TABLE_BYTES,
            })?;
        if table_len > X_PROPERTY_MAX_TABLE_BYTES {
            return Err(XPropertyError::TableTooLarge {
                len: table_len,
                max: X_PROPERTY_MAX_TABLE_BYTES,
            });
        }

        let record = XPropertyRecord {
            namespace,
            window: change.window,
            property: change.property,
            property_type: change.property_type,
            format: change.format,
            bytes,
            generation,
        };
        self.records.insert(key, record.clone());
        Ok(record)
    }

    pub fn get(
        &self,
        namespace: NamespaceId,
        window: XResourceId,
        property: XAtom,
    ) -> Option<&XPropertyRecord> {
        self.records.get(&(namespace, window, property))
    }

    pub fn properties_for_window(&self, namespace: NamespaceId, window: XResourceId) -> Vec<XAtom> {
        self.records
            .keys()
            .filter_map(|(record_namespace, record_window, property)| {
                (*record_namespace == namespace && *record_window == window).then_some(*property)
            })
            .collect()
    }

    pub fn windows_with_property(
        &self,
        namespace: NamespaceId,
        property: XAtom,
    ) -> Vec<XResourceId> {
        self.records
            .keys()
            .filter_map(|(record_namespace, window, record_property)| {
                (*record_namespace == namespace && *record_property == property).then_some(*window)
            })
            .collect()
    }

    pub fn remove_window(&mut self, namespace: NamespaceId, window: XResourceId) -> usize {
        let before = self.records.len();
        self.records
            .retain(|(record_namespace, record_window, _), _| {
                *record_namespace != namespace || *record_window != window
            });
        before.saturating_sub(self.records.len())
    }

    pub fn remove(
        &mut self,
        namespace: NamespaceId,
        window: XResourceId,
        property: XAtom,
    ) -> Option<XPropertyRecord> {
        self.records.remove(&(namespace, window, property))
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn read_property(
        &mut self,
        namespace: NamespaceId,
        read: XPropertyRead,
    ) -> Result<XPropertyReadOutcome, XPropertyError> {
        if !namespace.is_valid() {
            return Err(XPropertyError::InvalidNamespace);
        }
        if !read.window.is_valid() {
            return Err(XPropertyError::InvalidWindow);
        }

        let Some(record) = self.get(namespace, read.window, read.property) else {
            return Ok(XPropertyReadOutcome {
                reply: XPropertyReadReply {
                    property_type: X_PROPERTY_ANY_TYPE,
                    format: 0,
                    bytes_after: 0,
                    item_count: 0,
                    bytes: Vec::new(),
                },
                deleted: false,
            });
        };

        let reply = read_property_value(
            record.property_type,
            record.format,
            &record.bytes,
            read.property_type,
            read.long_offset,
            read.long_length,
        )?;
        let deleted = read.delete
            && reply.property_type != X_PROPERTY_ANY_TYPE
            && (read.property_type == X_PROPERTY_ANY_TYPE
                || read.property_type == reply.property_type)
            && reply.bytes_after == 0;
        if deleted {
            self.remove(namespace, read.window, read.property);
        }
        Ok(XPropertyReadOutcome { reply, deleted })
    }
}

pub(crate) fn read_property_value(
    property_type: XAtom,
    format: u8,
    bytes: &[u8],
    requested_type: XAtom,
    long_offset: u32,
    long_length: u32,
) -> Result<XPropertyReadReply, XPropertyError> {
    if requested_type != X_PROPERTY_ANY_TYPE && requested_type != property_type {
        return Ok(XPropertyReadReply {
            property_type,
            format,
            bytes_after: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            item_count: 0,
            bytes: Vec::new(),
        });
    }

    let offset = usize::try_from(long_offset)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .ok_or(XPropertyError::InvalidOffset)?;
    if offset > bytes.len() {
        return Err(XPropertyError::InvalidOffset);
    }

    let remaining = bytes.len() - offset;
    let requested_bytes = usize::try_from(long_length)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);
    let returned_len = remaining.min(requested_bytes);
    let bytes_after = remaining - returned_len;
    let item_width = usize::from(format / 8);
    Ok(XPropertyReadReply {
        property_type,
        format,
        bytes_after: u32::try_from(bytes_after).unwrap_or(u32::MAX),
        item_count: u32::try_from(returned_len / item_width).unwrap_or(u32::MAX),
        bytes: bytes[offset..offset + returned_len].to_vec(),
    })
}

pub(crate) fn validate_property_format(format: u8) -> Result<(), XPropertyError> {
    match format {
        8 | 16 | 32 => Ok(()),
        other => Err(XPropertyError::InvalidFormat(other)),
    }
}

fn ensure_same_property_shape(
    record: &XPropertyRecord,
    change: &XPropertyChange,
) -> Result<(), XPropertyError> {
    if record.property_type != change.property_type || record.format != change.format {
        return Err(XPropertyError::TypeMismatch);
    }
    Ok(())
}

fn joined_bytes(first: &[u8], second: &[u8]) -> Result<Vec<u8>, XPropertyError> {
    let len = first
        .len()
        .checked_add(second.len())
        .ok_or(XPropertyError::ValueTooLarge {
            len: usize::MAX,
            max: X_PROPERTY_MAX_VALUE_BYTES,
        })?;
    if len > X_PROPERTY_MAX_VALUE_BYTES {
        return Err(XPropertyError::ValueTooLarge {
            len,
            max: X_PROPERTY_MAX_VALUE_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(len);
    bytes.extend_from_slice(first);
    bytes.extend_from_slice(second);
    Ok(bytes)
}
