use super::{AuthorityKind, AuthorityLocalId};
use crate::geometry::{Rect, Region, Size, Transform};
use crate::ids::{NamespaceId, OutputId, SurfaceId, TransactionId, WorkspaceId, XWindowId};

mod authority_surface;
mod layout;
mod presentation_intent;
mod snapshot;
mod transaction;

pub use authority_surface::*;
pub use layout::*;
pub use presentation_intent::*;
pub use snapshot::*;
pub use transaction::*;
