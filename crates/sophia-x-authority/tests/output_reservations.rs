use sophia_protocol::{AxisSpan, NamespaceId, OutputEdge, OutputReservation, Rect};
use sophia_x_authority::{
    X_ATOM_CARDINAL, X_ATOM_NAME_NET_WM_STRUT, X_ATOM_NAME_NET_WM_STRUT_PARTIAL, XAtomTable,
    XByteOrder, XOutputReservationDecodeError, XPropertyChange, XPropertyMode, XPropertyRecord,
    XPropertyTable, XResourceId, decode_x_output_reservations, x_output_reservations_for_window,
};

const NAMESPACE: NamespaceId = NamespaceId::from_raw(1);
const WINDOW: XResourceId = XResourceId::new(0x20_0001, 1);
const ROOT: Rect = Rect {
    x: 0,
    y: 0,
    width: 1920,
    height: 1080,
};

#[test]
fn partial_strut_decodes_every_root_relative_edge() {
    let mut atoms = XAtomTable::new();
    let property = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT_PARTIAL);
    let record = record(
        property,
        &[12, 24, 36, 48, 0, 1079, 0, 1079, 0, 959, 960, 1919],
        XByteOrder::LittleEndian,
    );

    let reservations =
        decode_x_output_reservations(&record, &atoms, XByteOrder::LittleEndian, ROOT)
            .expect("recognized property")
            .expect("valid property");

    assert_eq!(
        reservations,
        vec![
            reservation(OutputEdge::Left, 12, 0, 1080),
            reservation(OutputEdge::Right, 24, 0, 1080),
            reservation(OutputEdge::Top, 36, 0, 960),
            reservation(OutputEdge::Bottom, 48, 960, 1920),
        ]
    );
}

#[test]
fn partial_strut_honors_big_endian_cardinals() {
    let mut atoms = XAtomTable::new();
    let property = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT_PARTIAL);
    let record = record(
        property,
        &[0, 0, 28, 0, 0, 0, 0, 0, 0, 1919, 0, 0],
        XByteOrder::BigEndian,
    );

    assert_eq!(
        decode_x_output_reservations(&record, &atoms, XByteOrder::BigEndian, ROOT),
        Some(Ok(vec![reservation(OutputEdge::Top, 28, 0, 1920)]))
    );
}

#[test]
fn malformed_partial_falls_back_to_valid_legacy_strut() {
    let mut atoms = XAtomTable::new();
    let partial = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT_PARTIAL);
    let legacy = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT);
    let mut properties = XPropertyTable::new();
    apply(
        &mut properties,
        partial,
        X_ATOM_CARDINAL,
        32,
        cardinal_bytes(&[0, 0, 40], XByteOrder::LittleEndian),
    );
    apply(
        &mut properties,
        legacy,
        X_ATOM_CARDINAL,
        32,
        cardinal_bytes(&[0, 0, 32, 0], XByteOrder::LittleEndian),
    );

    assert_eq!(
        x_output_reservations_for_window(
            &properties,
            &atoms,
            NAMESPACE,
            WINDOW,
            XByteOrder::LittleEndian,
            ROOT,
        ),
        vec![reservation(OutputEdge::Top, 32, 0, 1920)]
    );
}

#[test]
fn valid_zero_partial_takes_precedence_over_legacy_strut() {
    let mut atoms = XAtomTable::new();
    let partial = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT_PARTIAL);
    let legacy = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT);
    let mut properties = XPropertyTable::new();
    apply(
        &mut properties,
        partial,
        X_ATOM_CARDINAL,
        32,
        cardinal_bytes(&[0; 12], XByteOrder::LittleEndian),
    );
    apply(
        &mut properties,
        legacy,
        X_ATOM_CARDINAL,
        32,
        cardinal_bytes(&[0, 0, 32, 0], XByteOrder::LittleEndian),
    );

    assert!(
        x_output_reservations_for_window(
            &properties,
            &atoms,
            NAMESPACE,
            WINDOW,
            XByteOrder::LittleEndian,
            ROOT,
        )
        .is_empty()
    );
}

#[test]
fn invalid_type_format_length_and_ranges_are_rejected() {
    let mut atoms = XAtomTable::new();
    let property = atom(&mut atoms, X_ATOM_NAME_NET_WM_STRUT_PARTIAL);
    let mut invalid_type = record(
        property,
        &[0, 0, 28, 0, 0, 0, 0, 0, 0, 1919, 0, 0],
        XByteOrder::LittleEndian,
    );
    invalid_type.property_type = property;
    assert_eq!(
        decode_x_output_reservations(&invalid_type, &atoms, XByteOrder::LittleEndian, ROOT),
        Some(Err(XOutputReservationDecodeError::InvalidType))
    );

    let mut invalid_format = invalid_type.clone();
    invalid_format.property_type = X_ATOM_CARDINAL;
    invalid_format.format = 16;
    assert_eq!(
        decode_x_output_reservations(&invalid_format, &atoms, XByteOrder::LittleEndian, ROOT),
        Some(Err(XOutputReservationDecodeError::InvalidFormat))
    );

    let short = XPropertyRecord {
        format: 32,
        bytes: vec![0; 44],
        ..invalid_format.clone()
    };
    assert_eq!(
        decode_x_output_reservations(&short, &atoms, XByteOrder::LittleEndian, ROOT),
        Some(Err(XOutputReservationDecodeError::InvalidLength))
    );

    let out_of_range = record(
        property,
        &[0, 0, 28, 0, 0, 0, 0, 0, 0, 1920, 0, 0],
        XByteOrder::LittleEndian,
    );
    assert_eq!(
        decode_x_output_reservations(&out_of_range, &atoms, XByteOrder::LittleEndian, ROOT),
        Some(Err(XOutputReservationDecodeError::ValueOutOfRange))
    );
}

fn atom(atoms: &mut XAtomTable, name: &str) -> u32 {
    atoms
        .intern(name, false)
        .expect("valid atom")
        .expect("atom allocated")
}

fn record(property: u32, values: &[u32], byte_order: XByteOrder) -> XPropertyRecord {
    XPropertyRecord {
        namespace: NAMESPACE,
        window: WINDOW,
        property,
        property_type: X_ATOM_CARDINAL,
        format: 32,
        bytes: cardinal_bytes(values, byte_order),
        generation: 1,
    }
}

fn apply(
    properties: &mut XPropertyTable,
    property: u32,
    property_type: u32,
    format: u8,
    bytes: Vec<u8>,
) {
    properties
        .apply_change(
            NAMESPACE,
            XPropertyChange {
                mode: XPropertyMode::Replace,
                window: WINDOW,
                property,
                property_type,
                format,
                bytes,
            },
        )
        .expect("property accepted");
}

fn cardinal_bytes(values: &[u32], byte_order: XByteOrder) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| match byte_order {
            XByteOrder::LittleEndian => value.to_le_bytes(),
            XByteOrder::BigEndian => value.to_be_bytes(),
        })
        .collect()
}

const fn reservation(edge: OutputEdge, depth: i32, start: i32, end: i32) -> OutputReservation {
    OutputReservation {
        edge,
        depth,
        span: AxisSpan { start, end },
    }
}
