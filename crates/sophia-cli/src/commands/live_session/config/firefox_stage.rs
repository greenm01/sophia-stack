#[derive(Default)]
pub(super) struct FirefoxM8StageProof {
    baseline_title_bytes: [Option<usize>; 16],
    active_residue: Option<usize>,
    completed_stage: usize,
    promotion: bool,
}

impl FirefoxM8StageProof {
    const FULL_STAGES: [&'static str; 8] = [
        "loaded",
        "keyboard",
        "clipboard",
        "primary",
        "scroll",
        "resize",
        "refocus",
        "dialog",
    ];
    const PROMOTION_STAGES: [&'static str; 6] = [
        "loaded", "keyboard", "scroll", "layout", "refocus", "dialog",
    ];

    pub(super) fn promotion() -> Self {
        Self {
            promotion: true,
            ..Self::default()
        }
    }

    fn stages(&self) -> &'static [&'static str] {
        if self.promotion {
            &Self::PROMOTION_STAGES
        } else {
            &Self::FULL_STAGES
        }
    }

    fn source_stage_index(&self, proof_stage_index: usize) -> usize {
        if self.promotion && proof_stage_index >= 2 {
            proof_stage_index.saturating_add(2)
        } else {
            proof_stage_index
        }
    }

    pub(super) fn stage_count(&self) -> usize {
        self.stages().len()
    }

    pub(super) fn completed(&self) -> usize {
        self.completed_stage
    }

    pub(super) fn observe(
        &mut self,
        property_name: &str,
        byte_len: usize,
    ) -> Vec<(&'static str, usize, usize)> {
        if property_name != "_NET_WM_NAME" || byte_len == 0 || byte_len > 256 {
            return Vec::new();
        }
        let residue = byte_len % 16;
        if self.completed_stage == 0 {
            let Some(baseline) = self.baseline_title_bytes[residue] else {
                self.baseline_title_bytes[residue] = Some(byte_len);
                return Vec::new();
            };
            if byte_len == baseline.saturating_add(16) {
                self.active_residue = Some(residue);
                self.completed_stage = 2;
                return vec![
                    (self.stages()[0], 0, baseline),
                    (self.stages()[1], 1, byte_len),
                ];
            }
            if byte_len != baseline {
                self.baseline_title_bytes[residue] = Some(byte_len);
            }
            return Vec::new();
        }
        if self.completed_stage >= self.stage_count() {
            return Vec::new();
        }
        let active_residue = self
            .active_residue
            .expect("stage activation records a residue");
        if residue != active_residue {
            return Vec::new();
        }
        let baseline = self.baseline_title_bytes[active_residue]
            .expect("stage activation retains its baseline");
        let source_stage_index = self.source_stage_index(self.completed_stage);
        let expected = baseline.saturating_add(source_stage_index.saturating_mul(16));
        if byte_len != expected {
            return Vec::new();
        }
        let stage_index = self.completed_stage;
        self.completed_stage += 1;
        vec![(self.stages()[stage_index], stage_index, byte_len)]
    }

    pub(super) fn navigation_ready(&self, property_name: &str, byte_len: usize) -> bool {
        if property_name != "_NET_WM_NAME"
            || self.source_stage_index(self.completed_stage) != 4
        {
            return false;
        }
        let Some(active_residue) = self.active_residue else {
            return false;
        };
        let Some(baseline) = self.baseline_title_bytes[active_residue] else {
            return false;
        };
        byte_len == baseline.saturating_add(49)
    }

    pub(super) fn dialog_ready(&self, property_name: &str, byte_len: usize) -> bool {
        if property_name != "_NET_WM_NAME"
            || self.source_stage_index(self.completed_stage) != 7
        {
            return false;
        }
        let Some(active_residue) = self.active_residue else {
            return false;
        };
        let Some(baseline) = self.baseline_title_bytes[active_residue] else {
            return false;
        };
        byte_len == baseline.saturating_add(97)
    }

    pub(super) fn complete(&self) -> bool {
        self.completed_stage == self.stage_count()
    }
}
