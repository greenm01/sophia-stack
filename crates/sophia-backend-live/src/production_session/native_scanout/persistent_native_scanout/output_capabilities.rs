use super::*;

impl LiveProductionNativeScanout {
    pub fn output_capabilities(&self) -> std::io::Result<Vec<crate::LibdrmNativeOutputCapability>> {
        let mut capabilities = self
            .groups
            .iter()
            .map(|group| group.session.output_capabilities())
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        capabilities.sort_by_key(|capability| capability.output().raw());
        Ok(capabilities)
    }
}
