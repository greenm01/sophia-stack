//! The metadata chain, end to end, with the real types at every hop.
//!
//! `sophia-cli` is the only crate that depends on the authority, the broker, and the
//! engine at once, so it is where the three can be composed without any of them
//! depending on each other. Nothing here is session plumbing: it proves the pieces
//! fit, which is the claim that has to hold before anything hosts them.
//!
//! The chain is: an authority reduces its own property under the broker's rule, the
//! broker adds trust and an icon token, and Engine stores the result.

use sophia_broker::{MetadataBroker, MetadataBrokerCommand, MetadataBrokerEvent};
use sophia_engine::ChromeDescriptorTable;
use sophia_protocol::{MetadataDisclosure, NamespaceId, NamespaceProfile, SurfaceId, TrustLevel};
use sophia_x_authority::{XAtomTable, XPropertyRecord, XResourceId, reduce_metadata_property};

const SURFACE: SurfaceId = SurfaceId::new(6, 1);

fn property(atoms: &mut XAtomTable, name: &str, kind: &str, bytes: &[u8]) -> XPropertyRecord {
    let property = atoms
        .intern(name, false)
        .expect("atom name is valid")
        .expect("interning creates the atom");
    let property_type = atoms
        .intern(kind, false)
        .expect("atom name is valid")
        .expect("interning creates the atom");
    XPropertyRecord {
        namespace: NamespaceId::from_raw(3),
        window: XResourceId::new(0x22_0044, 1),
        property,
        property_type,
        format: 8,
        bytes: bytes.to_vec(),
        generation: 2,
    }
}

/// Drives one property through the whole chain and returns what Engine stored.
fn run_chain(
    disclosure: MetadataDisclosure,
    name: &str,
    kind: &str,
    bytes: &[u8],
) -> (ChromeDescriptorTable, MetadataBroker) {
    let mut atoms = XAtomTable::new();
    let mut broker = MetadataBroker::new();
    let mut table = ChromeDescriptorTable::default();

    broker
        .update(MetadataBrokerEvent::SurfaceAdmitted {
            surface: SURFACE,
            profile: NamespaceProfile::Confined,
        })
        .expect("admission succeeds");
    broker
        .set_disclosure(SURFACE, disclosure)
        .expect("the surface is admitted");

    // The authority reduces under the rule the broker just published, so the rule
    // reaches the reduction rather than the reduction guessing at it.
    let rule = broker.rule_for(SURFACE).expect("a rule was published");
    let record = property(&mut atoms, name, kind, bytes);
    let candidate = reduce_metadata_property(&record, &atoms, SURFACE, Some(rule));

    for command in broker
        .update(MetadataBrokerEvent::CandidateReduced(candidate))
        .expect("the candidate matches its rule")
    {
        if let MetadataBrokerCommand::EmitDescriptor { descriptor, .. } = command {
            table.apply_metadata(descriptor);
        }
    }
    (table, broker)
}

#[test]
fn a_title_reaches_engine_when_the_rule_permits_it() {
    let (table, _) = run_chain(
        MetadataDisclosure::Full,
        "_NET_WM_NAME",
        "UTF8_STRING",
        "Quarterly Review".as_bytes(),
    );

    let stored = table.get(SURFACE).expect("Engine stored a descriptor");
    assert_eq!(
        stored.label.as_ref().map(|label| label.text.as_str()),
        Some("Quarterly Review")
    );
    // Trust came from the namespace, which only the broker knows.
    assert_eq!(stored.trust_level, TrustLevel::Isolated);
    // The icon token came from the broker's allocator, not from the authority.
    assert!(stored.icon.is_some());
}

#[test]
fn a_title_never_reaches_engine_when_the_rule_forbids_it() {
    // The whole chain's reason for existing. The authority holds the title, the rule
    // says class only, and nothing downstream ever sees the text -- not the broker,
    // not Engine.
    let (table, _) = run_chain(
        MetadataDisclosure::ClassOnly,
        "_NET_WM_NAME",
        "UTF8_STRING",
        "Quarterly Salary Review.ods".as_bytes(),
    );

    let stored = table.get(SURFACE).expect("Engine stored a descriptor");
    assert_eq!(stored.label, None);
}

#[test]
fn a_class_reaches_engine_under_the_class_only_rule() {
    let (table, _) = run_chain(
        MetadataDisclosure::ClassOnly,
        "WM_CLASS",
        "STRING",
        b"soffice\0LibreOffice\0",
    );

    let stored = table.get(SURFACE).expect("Engine stored a descriptor");
    assert_eq!(
        stored.label.as_ref().map(|label| label.text.as_str()),
        Some("LibreOffice")
    );
}

#[test]
fn the_engine_ingress_needed_no_widening_for_the_broker() {
    // The plan's load-bearing claim: SanitizedChromeMetadata already described
    // everything the broker produces, so the broker was built to an existing
    // boundary rather than reshaping one to fit.
    let (table, broker) = run_chain(
        MetadataDisclosure::Full,
        "_NET_WM_NAME",
        "UTF8_STRING",
        "Report".as_bytes(),
    );

    assert_eq!(table.len(), 1);
    assert_eq!(broker.len(), 1);
    let stored = table.get(SURFACE).expect("Engine stored a descriptor");
    assert_eq!(stored.surface, SURFACE);
    assert_eq!(stored.generation, 2);
}
