impl XAuthorityRuntime {
    pub fn create_sync_counter(
        &mut self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
        generation: u64,
        initial_value: i64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .insert(
                counter,
                XResourceKind::SyncCounter,
                namespace,
                generation,
            )
            .map_err(XAuthorityRuntimeError::from)?;
        self.sync_counters.insert(counter, initial_value);
        Ok(())
    }

    pub fn set_sync_counter(
        &mut self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
        value: i64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_sync_counter_access(namespace, counter)?;
        *self
            .sync_counters
            .get_mut(&counter)
            .ok_or(XAuthorityRuntimeError::UnknownResource)? = value;
        Ok(())
    }

    pub fn change_sync_counter(
        &mut self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
        delta: i64,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_sync_counter_access(namespace, counter)?;
        let value = self
            .sync_counters
            .get_mut(&counter)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        *value = value.wrapping_add(delta);
        Ok(())
    }

    pub fn sync_counter(
        &self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
    ) -> Result<i64, XAuthorityRuntimeError> {
        self.validate_sync_counter_access(namespace, counter)?;
        self.sync_counters
            .get(&counter)
            .copied()
            .ok_or(XAuthorityRuntimeError::UnknownResource)
    }

    pub fn destroy_sync_counter(
        &mut self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.validate_sync_counter_access(namespace, counter)?;
        self.resources.remove(counter);
        self.sync_counters
            .remove(&counter)
            .ok_or(XAuthorityRuntimeError::UnknownResource)?;
        Ok(())
    }

    fn validate_sync_counter_access(
        &self,
        namespace: NamespaceId,
        counter: crate::XResourceId,
    ) -> Result<(), XAuthorityRuntimeError> {
        self.resources
            .lookup(namespace, counter, XResourceKind::SyncCounter)
            .map(|_| ())
            .map_err(Into::into)
    }
}
