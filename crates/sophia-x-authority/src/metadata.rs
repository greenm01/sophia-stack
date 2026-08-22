//! Reducing this authority's own metadata under the broker's disclosure rule.
//!
//! Raw titles, classes, PIDs, and paths are authority-private. They are inputs to
//! this reduction and to nothing else: the broker publishes a rule, this applies it
//! to text the authority already legitimately holds, and only the result leaves the
//! process. See `docs/sophia-x-authority.md` and `docs/style-guide.md`.
//!
//! The security property worth naming is narrow and easy to lose: `ClassOnly` must
//! never emit window-title content. A taskbar that groups by application has no
//! business learning what document is open, and the difference between the two is
//! one `match` arm.

use sophia_protocol::{
    DisplayLabel, MAX_CHROME_LABEL_LEN, MetadataDisclosure, MetadataDisclosureRule, NamespaceId,
    ReducedMetadataCandidate, SurfaceId,
};

use crate::atom::{
    X_ATOM_NAME_NET_WM_NAME, X_ATOM_NAME_UTF8_STRING, X_ATOM_NAME_WM_CLASS, X_ATOM_NAME_WM_NAME,
    XAtomTable,
};
use crate::property::XPropertyRecord;
use crate::{XPropertyTable, XResourceId};

/// Applies a disclosure rule to one retained property record.
///
/// Absent a rule the answer is `MetadataDisclosure::None`, so a surface the broker
/// has not ruled on discloses nothing. That default is deliberate: a new surface
/// exists before any rule can describe it, and the safe answer during that window
/// is silence.
///
/// A rule for a different surface is ignored rather than applied. Mismatched
/// identity is a caller error, and applying the wrong surface's permission is the
/// one failure mode worse than emitting no label.
pub fn reduce_metadata_property(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    surface: SurfaceId,
    rule: Option<MetadataDisclosureRule>,
) -> ReducedMetadataCandidate {
    let disclosure = rule
        .filter(|rule| rule.surface == surface)
        .map_or(MetadataDisclosure::None, |rule| rule.disclosure);

    ReducedMetadataCandidate {
        surface,
        label: reduce_label(record, atoms, disclosure),
        disclosure,
        generation: record.generation,
    }
}

/// Reduces one window's retained identity after any metadata property changes.
///
/// A descriptor is a view of the window, not of the last property packet. Under
/// `ClassOnly`, a later title update must therefore retain the class label rather
/// than replacing it with an empty descriptor. The candidate generation still
/// advances to the newest relevant property so broker ordering remains monotonic.
pub fn reduce_window_metadata(
    properties: &XPropertyTable,
    atoms: &XAtomTable,
    namespace: NamespaceId,
    window: XResourceId,
    surface: SurfaceId,
    rule: Option<MetadataDisclosureRule>,
) -> Option<ReducedMetadataCandidate> {
    let mut records = properties
        .properties_for_window(namespace, window)
        .into_iter()
        .filter_map(|property| properties.get(namespace, window, property))
        .filter(|record| {
            atoms
                .name(record.property)
                .is_some_and(crate::is_metadata_candidate_name)
        })
        .collect::<Vec<_>>();
    let generation = records.iter().map(|record| record.generation).max()?;
    let disclosure = rule
        .filter(|rule| rule.surface == surface)
        .map_or(MetadataDisclosure::None, |rule| rule.disclosure);
    let priority = |record: &&XPropertyRecord| match atoms.name(record.property) {
        Some(X_ATOM_NAME_NET_WM_NAME) if disclosure == MetadataDisclosure::Full => 0,
        Some(X_ATOM_NAME_WM_NAME) if disclosure == MetadataDisclosure::Full => 1,
        Some(X_ATOM_NAME_WM_CLASS) if disclosure.discloses_text() => 2,
        _ => 3,
    };
    records.sort_by_key(priority);
    let mut candidate = records
        .iter()
        .find_map(|record| {
            let candidate = reduce_metadata_property(record, atoms, surface, rule);
            candidate.label.is_some().then_some(candidate)
        })
        .unwrap_or_else(|| reduce_metadata_property(records[0], atoms, surface, rule));
    candidate.generation = generation;
    Some(candidate)
}

/// The whole disclosure decision, in one place.
///
/// Returning `None` covers three different situations on purpose — nothing was
/// permitted, the property carries no displayable identity, and the text failed
/// validation. None of them is actionable by a receiver, which needs to know only
/// that there is nothing to draw; the permitted *level* travels separately so a
/// receiver can still tell silence from an untitled window.
fn reduce_label(
    record: &XPropertyRecord,
    atoms: &XAtomTable,
    disclosure: MetadataDisclosure,
) -> Option<DisplayLabel> {
    if !disclosure.discloses_text() {
        return None;
    }
    let property_name = atoms.name(record.property)?;
    let text = match property_name {
        // A class is the least revealing identity a window has, so both levels that
        // disclose anything may carry it.
        X_ATOM_NAME_WM_CLASS => decode_wm_class(record, atoms)?,
        // A title is the window's own content. Only `Full` reaches it; `ClassOnly`
        // stops here, which is the rule this module exists to hold.
        X_ATOM_NAME_WM_NAME | X_ATOM_NAME_NET_WM_NAME if disclosure == MetadataDisclosure::Full => {
            decode_property_text(record, atoms)?
        }
        // Everything else, including WM_PROTOCOLS, carries no displayable identity.
        _ => return None,
    };
    bounded_label(&text)
}

/// `WM_CLASS` is `instance\0class\0`. Only the class is taken.
///
/// The instance is closer to a process identity than to an application one, and it
/// is exactly the kind of incidental detail that turns a label into a fingerprint.
fn decode_wm_class(record: &XPropertyRecord, atoms: &XAtomTable) -> Option<String> {
    let text = decode_property_text(record, atoms)?;
    let mut fields = text.split('\0').filter(|field| !field.is_empty());
    let instance = fields.next();
    fields.next().map(str::to_owned).or_else(|| {
        // A single field is ambiguous. Treating it as the class matches what X
        // clients that set only one string intend, and it is the less revealing
        // reading either way.
        instance.map(str::to_owned)
    })
}

/// Decodes property bytes by their declared type.
///
/// `UTF8_STRING` is UTF-8 and invalid sequences are refused rather than replaced,
/// because a replacement character in a label is a decoding bug wearing a costume.
/// Anything else is treated as Latin-1, which is what `STRING` means in X11 and the
/// only other encoding these properties use in practice.
fn decode_property_text(record: &XPropertyRecord, atoms: &XAtomTable) -> Option<String> {
    if record.format != 8 || record.bytes.is_empty() {
        return None;
    }
    match atoms.name(record.property_type) {
        Some(X_ATOM_NAME_UTF8_STRING) => String::from_utf8(record.bytes.clone()).ok(),
        _ => Some(record.bytes.iter().map(|byte| char::from(*byte)).collect()),
    }
}

/// Bounds and validates a label the same way Engine will when it arrives.
///
/// Truncation is on a character boundary and sets `redacted`, so a receiver can
/// tell a shortened label from a complete one. Control characters reject the whole
/// label rather than being stripped: a label is presentation text, and stripping
/// would silently produce something the client never set.
fn bounded_label(text: &str) -> Option<DisplayLabel> {
    let trimmed = text.trim_matches('\0');
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return None;
    }
    if trimmed.len() <= MAX_CHROME_LABEL_LEN {
        return Some(DisplayLabel {
            text: trimmed.to_owned(),
            redacted: false,
        });
    }
    let mut end = MAX_CHROME_LABEL_LEN;
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }
    (end > 0).then(|| DisplayLabel {
        text: trimmed[..end].to_owned(),
        redacted: true,
    })
}
