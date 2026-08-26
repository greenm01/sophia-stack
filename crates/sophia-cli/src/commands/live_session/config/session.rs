use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::{
    BROWSER_APPLICATION_ID, LAUNCHER_APPLICATION_ID, TERMINAL_APPLICATION_ID, WmSessionAction,
};

pub(super) fn session_action_evidence_name(action: WmSessionAction) -> &'static str {
    match action {
        WmSessionAction::LaunchApplication { application }
            if application == TERMINAL_APPLICATION_ID =>
        {
            "LaunchTerminal"
        }
        WmSessionAction::LaunchApplication { application }
            if application == LAUNCHER_APPLICATION_ID =>
        {
            "LaunchApplicationMenu"
        }
        WmSessionAction::LaunchApplication { application }
            if application == BROWSER_APPLICATION_ID =>
        {
            "LaunchBrowser"
        }
        WmSessionAction::LaunchApplication { .. } => "LaunchApplication",
        WmSessionAction::CloseFocused => "CloseFocused",
        WmSessionAction::Logout => "Logout",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionApplicationSpec {
    pub(super) id: String,
    pub(super) executable: std::path::PathBuf,
    pub(super) arguments: Vec<String>,
    pub(super) placement_classification: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionApplicationConfig {
    pub(super) applications: BTreeMap<String, SessionApplicationSpec>,
    pub(super) startup: Vec<String>,
    pub(super) terminal: Option<String>,
    pub(super) launcher: Option<String>,
    pub(super) browser: Option<String>,
    pub(super) logout_enabled: bool,
}

impl Default for SessionApplicationConfig {
    fn default() -> Self {
        Self {
            applications: BTreeMap::new(),
            startup: Vec::new(),
            terminal: None,
            launcher: None,
            browser: None,
            logout_enabled: true,
        }
    }
}

impl SessionApplicationConfig {
    fn application_for_profile_name(
        &self,
        name: &str,
    ) -> Result<Option<String>, SessionApplicationConfigError> {
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
            return Err(SessionApplicationConfigError::AmbiguousProfileIdentity(
                name.to_owned(),
            ));
        }
        Ok(selected)
    }

    pub(super) fn apply_desktop_candidate(
        &mut self,
        candidate: &sophia_config::DesktopSessionCandidate,
        terminal_overridden: bool,
        browser_overridden: bool,
        startup_overridden: bool,
    ) -> Result<(), SessionApplicationConfigError> {
        if !terminal_overridden
            && let Some(terminal) = candidate.terminal.as_deref()
        {
            self.terminal = self.application_for_profile_name(terminal)?;
        }
        if !browser_overridden
            && let Some(browser) = candidate.browser.as_deref()
        {
            self.browser = self.application_for_profile_name(browser)?;
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
        shell_enabled: bool,
    ) -> Result<(), SessionApplicationConfigError> {
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
                ) => self.browser.is_some(),
                sophia_config::DesktopShortcutTarget::Session(
                    sophia_config::DesktopSessionShortcut::WindowSwitcher,
                ) => shell_enabled,
            };
            if !available {
                return Err(SessionApplicationConfigError::UnavailableShortcutCapability);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SessionApplicationOverrides {
    additions: Vec<SessionApplicationSpec>,
    argument_extensions: Vec<(String, String)>,
    startup: Option<Vec<String>>,
    terminal: Option<String>,
    launcher: Option<String>,
    browser: Option<String>,
}

impl SessionApplicationOverrides {
    pub(super) fn parse(args: &[String]) -> Result<Self, SessionApplicationConfigError> {
        let mut additions = Vec::new();
        let mut addition_ids = BTreeSet::new();
        for value in args
            .iter()
            .filter_map(|argument| argument.strip_prefix("--session-app="))
        {
            let (id, executable) = value
                .split_once('=')
                .ok_or(SessionApplicationConfigError::InvalidCli(
                    "--session-app expects ID=/absolute/executable",
                ))?;
            validate_session_app_id(id)?;
            let executable = std::path::PathBuf::from(executable);
            if !executable.is_absolute() || executable.as_os_str().is_empty() {
                return Err(SessionApplicationConfigError::InvalidCli(
                    "--session-app executable must be an absolute path",
                ));
            }
            if !addition_ids.insert(id.to_owned()) {
                return Err(SessionApplicationConfigError::DuplicateApplication(
                    id.to_owned(),
                ));
            }
            additions.push(SessionApplicationSpec {
                id: id.to_owned(),
                executable,
                arguments: Vec::new(),
                placement_classification: None,
            });
        }

        let mut argument_extensions = Vec::new();
        for value in args
            .iter()
            .filter_map(|argument| argument.strip_prefix("--session-app-arg="))
        {
            let (id, argument) = value
                .split_once('=')
                .ok_or(SessionApplicationConfigError::InvalidCli(
                    "--session-app-arg expects ID=ARG",
                ))?;
            if argument.len() > 4_096 {
                return Err(SessionApplicationConfigError::InvalidCli(
                    "--session-app-arg accepts at most 4096 bytes",
                ));
            }
            argument_extensions.push((id.to_owned(), argument.to_owned()));
        }

        let startup_values = args
            .iter()
            .filter_map(|argument| argument.strip_prefix("--session-start="))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let startup = if startup_values.is_empty() {
            None
        } else {
            let mut unique = BTreeSet::new();
            for id in &startup_values {
                validate_session_app_id(id)?;
                if !unique.insert(id.clone()) {
                    return Err(SessionApplicationConfigError::DuplicateStartup(id.clone()));
                }
            }
            Some(startup_values)
        };

        let mut terminal = None;
        let mut launcher = None;
        let mut browser = None;
        for value in args
            .iter()
            .filter_map(|argument| argument.strip_prefix("--session-action-app="))
        {
            let (action, id) = value
                .split_once('=')
                .ok_or(SessionApplicationConfigError::InvalidCli(
                    "--session-action-app expects terminal|launcher|browser=ID",
                ))?;
            let slot = match action {
                "terminal" => &mut terminal,
                "launcher" => &mut launcher,
                "browser" => &mut browser,
                _ => {
                    return Err(SessionApplicationConfigError::InvalidCli(
                        "--session-action-app expects terminal, launcher, or browser",
                    ));
                }
            };
            if slot.replace(id.to_owned()).is_some() {
                return Err(SessionApplicationConfigError::DuplicateAction(
                    action.to_owned(),
                ));
            }
        }

        Ok(Self {
            additions,
            argument_extensions,
            startup,
            terminal,
            launcher,
            browser,
        })
    }

    pub(super) fn prepare(
        &self,
        mut applications: SessionApplicationConfig,
        candidate: &sophia_config::DesktopSessionCandidate,
    ) -> Result<SessionApplicationConfig, SessionApplicationConfigError> {
        for addition in &self.additions {
            if applications.applications.len() >= 32 {
                return Err(SessionApplicationConfigError::ApplicationLimit);
            }
            if applications
                .applications
                .insert(addition.id.clone(), addition.clone())
                .is_some()
            {
                return Err(SessionApplicationConfigError::DuplicateApplication(
                    addition.id.clone(),
                ));
            }
        }
        for (id, argument) in &self.argument_extensions {
            let application = applications
                .applications
                .get_mut(id)
                .ok_or_else(|| SessionApplicationConfigError::UnknownApplication(id.clone()))?;
            if application.arguments.len() >= 32 {
                return Err(SessionApplicationConfigError::ArgumentLimit(id.clone()));
            }
            application.arguments.push(argument.clone());
        }

        applications.apply_desktop_candidate(
            candidate,
            self.terminal.is_some(),
            self.browser.is_some(),
            self.startup.is_some(),
        )?;
        if let Some(startup) = &self.startup {
            for id in startup {
                require_application(&applications, id)?;
            }
            applications.startup.clone_from(startup);
        }
        for id in [&self.terminal, &self.launcher, &self.browser]
            .into_iter()
            .flatten()
        {
            require_application(&applications, id)?;
        }
        if let Some(terminal) = &self.terminal {
            applications.terminal = Some(terminal.clone());
        }
        if let Some(launcher) = &self.launcher {
            applications.launcher = Some(launcher.clone());
        }
        if let Some(browser) = &self.browser {
            applications.browser = Some(browser.clone());
        }
        Ok(applications)
    }
}

fn validate_session_app_id(id: &str) -> Result<(), SessionApplicationConfigError> {
    if id.is_empty()
        || id.len() > 32
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
        })
    {
        return Err(SessionApplicationConfigError::InvalidCli(
            "session application IDs accept 1-32 lowercase ASCII letters, digits, '-' or '_'",
        ));
    }
    Ok(())
}

fn require_application(
    applications: &SessionApplicationConfig,
    id: &str,
) -> Result<(), SessionApplicationConfigError> {
    if applications.applications.contains_key(id) {
        Ok(())
    } else {
        Err(SessionApplicationConfigError::UnknownApplication(
            id.to_owned(),
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SessionApplicationConfigError {
    InvalidCli(&'static str),
    ApplicationLimit,
    ArgumentLimit(String),
    DuplicateApplication(String),
    DuplicateStartup(String),
    DuplicateAction(String),
    UnknownApplication(String),
    AmbiguousProfileIdentity(String),
    UnavailableShortcutCapability,
}

impl fmt::Display for SessionApplicationConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCli(message) => formatter.write_str(message),
            Self::ApplicationLimit => {
                formatter.write_str("--session-app accepts at most 32 applications")
            }
            Self::ArgumentLimit(id) => {
                write!(formatter, "session app {id:?} accepts at most 32 arguments")
            }
            Self::DuplicateApplication(id) => write!(formatter, "duplicate --session-app ID {id:?}"),
            Self::DuplicateStartup(id) => write!(formatter, "duplicate --session-start ID {id:?}"),
            Self::DuplicateAction(action) => {
                write!(formatter, "duplicate session action mapping {action:?}")
            }
            Self::UnknownApplication(id) => {
                write!(formatter, "session configuration references unknown app {id:?}")
            }
            Self::AmbiguousProfileIdentity(name) => write!(
                formatter,
                "desktop session application identity {name:?} is ambiguous"
            ),
            Self::UnavailableShortcutCapability => formatter
                .write_str("desktop shortcut references an unavailable session capability"),
        }
    }
}

impl std::error::Error for SessionApplicationConfigError {}
