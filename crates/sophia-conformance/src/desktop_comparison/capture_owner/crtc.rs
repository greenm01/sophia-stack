//! Resolve the tracepoint CRTC index for the contracted DP-1 connector.

use drm::control::{Device as _, connector};
use std::fs;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::{Path, PathBuf};

struct Card(fs::File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}
impl drm::control::Device for Card {}

pub(super) fn resolve_dp1_crtc() -> Result<u64, String> {
    let mut cards = Vec::new();
    for entry in fs::read_dir("/sys/class/drm")
        .map_err(|error| format!("could not enumerate DRM connectors: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not inspect DRM connector: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(card) = name.strip_suffix("-DP-1") else {
            continue;
        };
        let status = fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        if status.trim() == "connected" {
            cards.push(PathBuf::from("/dev/dri").join(card));
        }
    }
    if cards.len() != 1 {
        return Err(format!(
            "comparison requires exactly one connected DP-1 connector; found {}",
            cards.len()
        ));
    }
    let target = &cards[0];
    let expected = resolve_card_dp1(target)?;
    reject_cross_card_crtc_alias(target, expected)?;
    Ok(expected)
}

/// The tracepoint exposes a card-local CRTC index without a device identity.
/// A second active card using the same index would make its completions
/// indistinguishable from DP-1, so the comparison must fail closed.
fn reject_cross_card_crtc_alias(target: &Path, expected: u64) -> Result<(), String> {
    for entry in fs::read_dir("/sys/class/drm")
        .map_err(|error| format!("could not enumerate DRM cards: {error}"))?
    {
        let entry = entry.map_err(|error| format!("could not inspect DRM card: {error}"))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !name.strip_prefix("card").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        }) {
            continue;
        }
        let path = PathBuf::from("/dev/dri").join(name);
        if path == target {
            continue;
        }
        let card = Card(fs::File::open(&path).map_err(|error| {
            format!(
                "could not open {} for CRTC ambiguity check: {error}",
                path.display()
            )
        })?);
        let resources = card
            .resource_handles()
            .map_err(|error| format!("could not read {} DRM resources: {error}", path.display()))?;
        for handle in resources.connectors() {
            let Ok(connector) = card.get_connector(*handle, false) else {
                continue;
            };
            if connector.state() != connector::State::Connected {
                continue;
            }
            let Some(encoder) = connector.current_encoder() else {
                continue;
            };
            let Some(crtc) = card
                .get_encoder(encoder)
                .map_err(|error| format!("could not inspect active encoder: {error}"))?
                .crtc()
            else {
                continue;
            };
            let index = resources
                .crtcs()
                .iter()
                .position(|candidate| *candidate == crtc)
                .and_then(|index| u64::try_from(index).ok());
            if index == Some(expected) {
                return Err(format!(
                    "kernel DRM timing is ambiguous: {} also has active CRTC index {expected}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn resolve_card_dp1(path: &Path) -> Result<u64, String> {
    let card = Card(fs::File::open(path).map_err(|error| {
        format!(
            "could not open {} for DRM discovery: {error}",
            path.display()
        )
    })?);
    let resources = card
        .resource_handles()
        .map_err(|error| format!("could not read {} DRM resources: {error}", path.display()))?;
    let mut connectors = resources
        .connectors()
        .iter()
        .filter_map(|handle| card.get_connector(*handle, false).ok())
        .filter(|info| {
            info.interface() == connector::Interface::DisplayPort
                && info.interface_id() == 1
                && info.state() == connector::State::Connected
        });
    let connector = connectors
        .next()
        .ok_or_else(|| format!("{} does not expose connected DP-1", path.display()))?;
    if connectors.next().is_some() {
        return Err(format!(
            "{} exposes duplicate DP-1 connectors",
            path.display()
        ));
    }
    let encoder = connector
        .current_encoder()
        .ok_or("connected DP-1 has no active encoder")?;
    let crtc = card
        .get_encoder(encoder)
        .map_err(|error| format!("could not inspect DP-1 encoder: {error}"))?
        .crtc()
        .ok_or("connected DP-1 encoder has no active CRTC")?;
    resources
        .crtcs()
        .iter()
        .position(|candidate| *candidate == crtc)
        .and_then(|index| u64::try_from(index).ok())
        .ok_or_else(|| "DP-1 CRTC is absent from the DRM resource index".to_owned())
}
