#[cfg(unix)]
impl XServerFrontendRouteRegistry {
    fn select_core_events(
        &self,
        client: XServerFrontendClientId,
        window: XResourceId,
        mask: u32,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut subscriptions = self
            .core_event_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let key = (client, window);
        if mask == 0 {
            subscriptions.remove(&key);
        } else {
            subscriptions.insert(key, mask);
        }
        Ok(())
    }

    fn remove_core_event_window(
        &self,
        window: XResourceId,
    ) -> Result<(), XServerFrontendRouteError> {
        self.core_event_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .retain(|(_, candidate), _| *candidate != window);
        Ok(())
    }

    fn property_change_subscribers(
        &self,
        window: XResourceId,
    ) -> Result<Vec<XServerFrontendClientId>, XServerFrontendRouteError> {
        const PROPERTY_CHANGE_MASK: u32 = 1 << 22;
        Ok(self
            .core_event_subscriptions
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .iter()
            .filter_map(|((client, candidate), mask)| {
                (*candidate == window && *mask & PROPERTY_CHANGE_MASK != 0).then_some(*client)
            })
            .collect())
    }
}
