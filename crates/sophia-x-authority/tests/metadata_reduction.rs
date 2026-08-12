use sophia_protocol::{
    MetadataDisclosure, MetadataDisclosureRule, NamespaceId, SurfaceId, TrustLevel,
};
use sophia_x_authority::{XAtomTable, XPropertyRecord, XResourceId, reduce_metadata_property};

const SURFACE: SurfaceId = SurfaceId::new(9, 1);

fn atoms() -> XAtomTable {
    XAtomTable::new()
}

fn record(
    atoms: &mut XAtomTable,
    property: &str,
    property_type: &str,
    bytes: &[u8],
) -> XPropertyRecord {
    let property = atoms
        .intern(property, false)
        .expect("atom name is valid")
        .expect("interning creates the atom");
    let property_type = atoms
        .intern(property_type, false)
        .expect("atom name is valid")
        .expect("interning creates the atom");
    XPropertyRecord {
        namespace: NamespaceId::from_raw(7),
        window: XResourceId::new(0x22_0010, 1),
        property,
        property_type,
        format: 8,
        bytes: bytes.to_vec(),
        generation: 4,
    }
}

fn rule(disclosure: MetadataDisclosure) -> MetadataDisclosureRule {
    MetadataDisclosureRule {
        surface: SURFACE,
        disclosure,
        trust_level: TrustLevel::Unknown,
        icon: None,
        generation: 1,
    }
}

fn label_text(
    atoms: &mut XAtomTable,
    property: &str,
    property_type: &str,
    bytes: &[u8],
    disclosure: MetadataDisclosure,
) -> Option<String> {
    let record = record(atoms, property, property_type, bytes);
    reduce_metadata_property(&record, atoms, SURFACE, Some(rule(disclosure)))
        .label
        .map(|label| label.text)
}

#[test]
fn class_only_never_emits_window_title_content() {
    // The security property this module exists to hold, and the one a reviewer
    // cannot see by reading a diff. A taskbar grouping by application has no
    // business learning which document is open.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "WM_NAME",
            "STRING",
            b"Quarterly Salary Review.ods",
            MetadataDisclosure::ClassOnly,
        ),
        None
    );
    assert_eq!(
        label_text(
            &mut atoms,
            "_NET_WM_NAME",
            "UTF8_STRING",
            "Quarterly Salary Review.ods".as_bytes(),
            MetadataDisclosure::ClassOnly,
        ),
        None
    );
}

#[test]
fn class_only_emits_the_class_and_never_the_instance() {
    // WM_CLASS is instance\0class\0. The instance is closer to a process identity
    // than an application one, and it is the incidental detail that turns a label
    // into a fingerprint.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "WM_CLASS",
            "STRING",
            b"soffice\0LibreOffice\0",
            MetadataDisclosure::ClassOnly,
        ),
        Some("LibreOffice".to_owned())
    );
}

#[test]
fn full_reaches_the_title() {
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "_NET_WM_NAME",
            "UTF8_STRING",
            "Quarterly Review".as_bytes(),
            MetadataDisclosure::Full,
        ),
        Some("Quarterly Review".to_owned())
    );
}

#[test]
fn a_surface_without_a_rule_discloses_nothing() {
    // A surface exists before any rule can describe it. Silence is the safe answer
    // during that window, and it must not depend on a caller remembering to ask.
    let mut atoms = atoms();
    let record = record(&mut atoms, "_NET_WM_NAME", "UTF8_STRING", b"Secret");

    let reduced = reduce_metadata_property(&record, &atoms, SURFACE, None);

    assert_eq!(reduced.label, None);
    assert_eq!(reduced.disclosure, MetadataDisclosure::None);
}

#[test]
fn a_rule_for_a_different_surface_is_not_applied() {
    // Applying another surface's permission is the one failure worse than emitting
    // no label, so a mismatch falls back to disclosing nothing.
    let mut atoms = atoms();
    let record = record(&mut atoms, "_NET_WM_NAME", "UTF8_STRING", b"Secret");
    let other = MetadataDisclosureRule {
        surface: SurfaceId::new(11, 1),
        disclosure: MetadataDisclosure::Full,
        trust_level: TrustLevel::Trusted,
        icon: None,
        generation: 1,
    };

    let reduced = reduce_metadata_property(&record, &atoms, SURFACE, Some(other));

    assert_eq!(reduced.label, None);
    assert_eq!(reduced.disclosure, MetadataDisclosure::None);
}

#[test]
fn an_over_long_label_is_truncated_on_a_character_boundary_and_marked() {
    // Multi-byte characters straddling the bound must not produce a partial
    // sequence, and a shortened label has to announce itself or a receiver will
    // treat it as the client's chosen name.
    let mut atoms = atoms();
    let text = "é".repeat(200);
    let record = record(&mut atoms, "_NET_WM_NAME", "UTF8_STRING", text.as_bytes());

    let label = reduce_metadata_property(
        &record,
        &atoms,
        SURFACE,
        Some(rule(MetadataDisclosure::Full)),
    )
    .label
    .expect("a long title still yields a label");

    assert!(label.redacted);
    assert!(label.text.len() <= 128);
    assert!(label.text.chars().all(|character| character == 'é'));
}

#[test]
fn control_characters_reject_the_whole_label() {
    // Stripping would silently produce a name the client never set, which is worse
    // than showing nothing.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "_NET_WM_NAME",
            "UTF8_STRING",
            b"Quarterly\x07Review",
            MetadataDisclosure::Full,
        ),
        None
    );
}

#[test]
fn invalid_utf8_is_refused_rather_than_replaced() {
    // A replacement character in a label is a decoding bug wearing a costume.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "_NET_WM_NAME",
            "UTF8_STRING",
            &[0xff, 0xfe, 0x41],
            MetadataDisclosure::Full,
        ),
        None
    );
}

#[test]
fn latin1_string_properties_decode_without_a_utf8_type() {
    // STRING is Latin-1 in X11, and it is still what many clients set.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "WM_NAME",
            "STRING",
            &[0x43, 0x61, 0x66, 0xe9],
            MetadataDisclosure::Full,
        ),
        Some("Café".to_owned())
    );
}

#[test]
fn properties_without_displayable_identity_never_produce_a_label() {
    // WM_PROTOCOLS is a metadata candidate for other reasons and carries no name.
    let mut atoms = atoms();

    assert_eq!(
        label_text(
            &mut atoms,
            "WM_PROTOCOLS",
            "ATOM",
            b"WM_DELETE_WINDOW",
            MetadataDisclosure::Full,
        ),
        None
    );
}

#[test]
fn the_reduced_candidate_carries_the_property_generation() {
    // Stale reductions must be rejectable downstream, which needs the generation the
    // property was retained at rather than the rule's.
    let mut atoms = atoms();
    let record = record(&mut atoms, "WM_CLASS", "STRING", b"soffice\0LibreOffice\0");

    let reduced = reduce_metadata_property(
        &record,
        &atoms,
        SURFACE,
        Some(rule(MetadataDisclosure::Full)),
    );

    assert_eq!(reduced.generation, 4);
    assert_eq!(reduced.surface, SURFACE);
}
