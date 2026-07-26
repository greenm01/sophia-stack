impl LiveWmSession {
    fn service_policy_update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.degraded || self.force_transport_restart {
            return Ok(());
        }
        if self.pending_policy_update.is_none() {
            let event = match self
                .transport
                .as_ref()
                .ok_or("WM transport is unavailable")?
                .try_policy_event()
            {
                Ok(event) => event,
                Err(WmTransportSubmitError::Disconnected) => {
                    self.request_transport_restart("policy_channel_disconnected", None);
                    return Ok(());
                }
                Err(WmTransportSubmitError::Busy) => {
                    return Err("WM transport returned an invalid busy policy event".into());
                }
            };
            match event {
                Some(WmTransportPolicyEvent::Update(update)) => {
                    self.pending_policy_update = Some(update);
                }
                Some(WmTransportPolicyEvent::Failed(error)) => {
                    self.request_transport_restart("policy_transport_failed", Some(&error));
                    return Ok(());
                }
                None => return Ok(()),
            }
        }

        let update = self
            .pending_policy_update
            .as_ref()
            .expect("pending WM policy update was populated");
        let outcome = self
            .shortcuts
            .as_mut()
            .ok_or("WM shortcut router is unavailable")?
            .apply_policy_update(update);
        let WmPolicyApplyOutcome::Acknowledged(acknowledgement) = outcome else {
            return Ok(());
        };
        let applied = acknowledgement.outcome == WmPolicyAckOutcome::Applied;
        match self
            .transport
            .as_ref()
            .ok_or("WM transport is unavailable")?
            .try_acknowledge_policy(acknowledgement)
        {
            Ok(()) => {}
            Err(WmTransportSubmitError::Busy) => {
                return Err("WM policy acknowledgement queue is unexpectedly full".into());
            }
            Err(WmTransportSubmitError::Disconnected) => {
                self.request_transport_restart("policy_ack_disconnected", None);
                return Ok(());
            }
        }
        if applied {
            let shortcuts = self
                .shortcuts
                .as_ref()
                .expect("WM shortcut router was available for policy application");
            let policy_generation = shortcuts.policy_generation();
            let binding_count = shortcuts.binding_count();
            self.chrome = shortcuts.chrome();
            if self.wm_chrome_supported {
                self.stage_visual_chrome(
                    PersistentXtermSessionConfig::wm_surface_chrome_style(self.chrome),
                );
            }
            println!(
                "sophia_live_wm_policy schema=2 status=applied generation={} bindings={} focus_ring_enabled={} focus_ring_width={} frame_enabled={} frame_width={} clearance={}",
                policy_generation,
                binding_count,
                self.chrome.focus_ring.enabled,
                self.chrome.focus_ring.width,
                self.chrome.frame.enabled,
                self.chrome.frame.width,
                self.chrome.clearance(),
            );
        }
        self.pending_policy_update = None;
        Ok(())
    }
}
