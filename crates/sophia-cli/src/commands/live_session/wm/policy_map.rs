#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LivePolicyMapMode {
    Direct,
    Deferred,
}

impl LivePolicyMapMode {
    const fn from_external_wm(external_wm_present: bool) -> Self {
        if external_wm_present {
            Self::Deferred
        } else {
            Self::Direct
        }
    }

    const fn frontend_deferred(self) -> bool {
        matches!(self, Self::Deferred)
    }

    const fn bypass_engine_admission(self) -> bool {
        matches!(self, Self::Direct)
    }
}
