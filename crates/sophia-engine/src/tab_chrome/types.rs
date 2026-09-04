use crate::{CompositorDisplayCommand, PresentedChromeTarget};
use sophia_protocol::{OutputId, PolicyTabGroup, Rect};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabBarProjection {
    pub policy: PolicyTabGroup,
    pub output: OutputId,
    pub group: u64,
    pub generation: u64,
    pub geometry: Rect,
    pub commands: Vec<CompositorDisplayCommand>,
    pub targets: Vec<PresentedChromeTarget>,
}
