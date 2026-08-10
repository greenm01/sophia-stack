use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionApplicationSpec {
    pub(super) id: String,
    pub(super) executable: std::path::PathBuf,
    pub(super) arguments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionApplicationConfig {
    pub(super) applications: BTreeMap<String, SessionApplicationSpec>,
    pub(super) startup: Vec<String>,
    pub(super) terminal: Option<String>,
    pub(super) launcher: Option<String>,
    pub(super) firefox: Option<String>,
    pub(super) logout_enabled: bool,
}

impl Default for SessionApplicationConfig {
    fn default() -> Self {
        Self {
            applications: BTreeMap::new(),
            startup: Vec::new(),
            terminal: None,
            launcher: None,
            firefox: None,
            logout_enabled: true,
        }
    }
}

impl SessionApplicationConfig {
    fn application_for_profile_name(
        &self,
        name: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        if self.applications.contains_key(name) {
            return Ok(Some(name.to_owned()));
        }
        let mut matches = self
            .applications
            .values()
            .filter(|application| {
                application
                    .executable
                    .file_name()
                    .is_some_and(|file| file == std::ffi::OsStr::new(name))
            })
            .map(|application| application.id.clone());
        let selected = matches.next();
        if matches.next().is_some() {
            return Err(format!(
                "desktop session application identity {name:?} is ambiguous"
            )
            .into());
        }
        Ok(selected)
    }

    pub(super) fn apply_desktop_candidate(
        &mut self,
        candidate: &sophia_config::DesktopSessionCandidate,
        terminal_overridden: bool,
        browser_overridden: bool,
        startup_overridden: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !terminal_overridden
            && let Some(terminal) = candidate.terminal.as_deref()
        {
            self.terminal = self.application_for_profile_name(terminal)?;
        }
        if !browser_overridden
            && let Some(browser) = candidate.browser.as_deref()
        {
            self.firefox = self.application_for_profile_name(browser)?;
        }
        if !startup_overridden
            && let Some(startup) = candidate.startup.as_deref()
        {
            self.startup = self
                .application_for_profile_name(startup)?
                .into_iter()
                .collect();
        }
        if let Some(enabled) = candidate.logout_enabled {
            self.logout_enabled = enabled;
        }
        Ok(())
    }

    pub(super) fn validate_shortcuts(
        &self,
        shortcuts: &sophia_config::DesktopShortcutCandidate,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for binding in &shortcuts.bindings {
            let available = match binding.target {
                sophia_config::DesktopShortcutTarget::PolicyAction(_) => true,
                sophia_config::DesktopShortcutTarget::Session(
                    sophia_config::DesktopSessionShortcut::CloseFocused,
                ) => true,
                sophia_config::DesktopShortcutTarget::Session(
                    sophia_config::DesktopSessionShortcut::Logout,
                ) => self.logout_enabled,
                sophia_config::DesktopShortcutTarget::Session(
                    sophia_config::DesktopSessionShortcut::LaunchTerminal,
                ) => self.terminal.is_some(),
                sophia_config::DesktopShortcutTarget::Session(
                    sophia_config::DesktopSessionShortcut::LaunchBrowser,
                ) => self.firefox.is_some(),
            };
            if !available {
                return Err("desktop shortcut references an unavailable session capability".into());
            }
        }
        Ok(())
    }
}
