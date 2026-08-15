use sophia_engine::RenderHeadId;
use sophia_protocol::OutputId;

/// Private mapping from one minted head to the physical facts behind it.
///
/// This record is the backend's side of the head boundary: Engine carries the
/// opaque `RenderHeadId`, and only this table can answer which card, connector,
/// or CRTC that head names. The mapping never crosses into Engine records,
/// policy IPC, or protocol packets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveNativeHeadRecord {
    pub head: RenderHeadId,
    pub output: OutputId,
    pub card_index: usize,
    pub connector_id: u32,
    pub crtc_id: u32,
    pub connector_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveNativeHeadTableError {
    InvalidHead,
    DuplicateHead {
        head: RenderHeadId,
    },
    /// One physical connector cannot stand behind two heads; admitting it
    /// twice would let two scanout lanes claim one CRTC.
    DuplicateConnector {
        card_index: usize,
        connector_id: u32,
    },
}

impl std::fmt::Display for LiveNativeHeadTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHead => write!(formatter, "native head table rejected an invalid head id"),
            Self::DuplicateHead { head } => write!(
                formatter,
                "native head table already contains head {}",
                head.raw()
            ),
            Self::DuplicateConnector {
                card_index,
                connector_id,
            } => write!(
                formatter,
                "native head table already maps card {card_index} connector {connector_id}"
            ),
        }
    }
}

impl std::error::Error for LiveNativeHeadTableError {}

/// The backend-owned head table for one live session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LiveProductionNativeHeadTable {
    records: Vec<LiveNativeHeadRecord>,
}

impl LiveProductionNativeHeadTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_records(
        records: impl IntoIterator<Item = LiveNativeHeadRecord>,
    ) -> Result<Self, LiveNativeHeadTableError> {
        let mut table = Self::new();
        for record in records {
            table.admit(record)?;
        }
        Ok(table)
    }

    pub fn admit(&mut self, record: LiveNativeHeadRecord) -> Result<(), LiveNativeHeadTableError> {
        if !record.head.is_valid() {
            return Err(LiveNativeHeadTableError::InvalidHead);
        }
        if self.records.iter().any(|known| known.head == record.head) {
            return Err(LiveNativeHeadTableError::DuplicateHead { head: record.head });
        }
        if self.records.iter().any(|known| {
            known.card_index == record.card_index && known.connector_id == record.connector_id
        }) {
            return Err(LiveNativeHeadTableError::DuplicateConnector {
                card_index: record.card_index,
                connector_id: record.connector_id,
            });
        }
        self.records.push(record);
        Ok(())
    }

    pub fn remove(&mut self, head: RenderHeadId) -> Option<LiveNativeHeadRecord> {
        let index = self.records.iter().position(|record| record.head == head)?;
        Some(self.records.remove(index))
    }

    pub fn head(&self, head: RenderHeadId) -> Option<&LiveNativeHeadRecord> {
        self.records.iter().find(|record| record.head == head)
    }

    pub fn crtc_to_head(&self, card_index: usize, crtc_id: u32) -> Option<RenderHeadId> {
        self.records
            .iter()
            .find(|record| record.card_index == card_index && record.crtc_id == crtc_id)
            .map(|record| record.head)
    }

    pub fn connector_to_head(&self, card_index: usize, connector_id: u32) -> Option<RenderHeadId> {
        self.records
            .iter()
            .find(|record| record.card_index == card_index && record.connector_id == connector_id)
            .map(|record| record.head)
    }

    pub fn records(&self) -> &[LiveNativeHeadRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
