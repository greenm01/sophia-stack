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
    resolve_card_dp1(&cards[0])
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
