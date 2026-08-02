#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum X11RoutedControl {
    Authority {
        command: XAuthorityControlCommand,
        focus: Option<X11FocusTransition>,
    },
    FocusOut {
        window: XResourceId,
    },
}

#[cfg(all(unix, test))]
impl X11RoutedControl {
    const fn authority_command(self) -> Option<XAuthorityControlCommand> {
        match self {
            Self::Authority { command, .. } => Some(command),
            Self::FocusOut { .. } => None,
        }
    }
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum X11FocusTransition {
    Unchanged,
    Enter { previous: Option<XResourceId> },
    Clear { previous: Option<XResourceId> },
}

#[cfg(unix)]
impl XServerFrontendRouteRegistry {
    fn route_focus_control(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Option<Result<(), XServerFrontendRouteError>> {
        match route.command {
            XAuthorityControlCommand::FocusSurface { surface, .. } => {
                Some(self.route_focus_surface(route, surface))
            }
            XAuthorityControlCommand::ClearFocus { .. } => Some(self.route_clear_focus(route)),
            _ => None,
        }
    }

    fn route_focus_surface(
        &self,
        route: XAuthorityClientControlCommand,
        surface: SurfaceId,
    ) -> Result<(), XServerFrontendRouteError> {
        let target = self
            .surfaces
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?
            .get(&surface)
            .copied();
        let Some(target) = target else {
            return self.route_authority_control(route, None);
        };
        if target.client != route.client {
            return Err(XServerFrontendRouteError::UnknownClient {
                client: route.client,
            });
        }
        let mut focused = self
            .focused_surface
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let transition = match *focused {
            Some(previous) if previous == target => X11FocusTransition::Unchanged,
            Some(previous) if previous.client == target.client => X11FocusTransition::Enter {
                previous: Some(previous.window),
            },
            Some(previous) => {
                self.route_focus_out(previous)?;
                X11FocusTransition::Enter { previous: None }
            }
            None => X11FocusTransition::Enter { previous: None },
        };
        self.route_authority_control(route, Some(transition))?;
        *focused = Some(target);
        Ok(())
    }

    fn route_clear_focus(
        &self,
        route: XAuthorityClientControlCommand,
    ) -> Result<(), XServerFrontendRouteError> {
        let mut focused = self
            .focused_surface
            .lock()
            .map_err(|_| XServerFrontendRouteError::RegistryPoisoned)?;
        let transition = match *focused {
            Some(previous) if previous.client == route.client => X11FocusTransition::Clear {
                previous: Some(previous.window),
            },
            Some(previous) => {
                self.route_focus_out(previous)?;
                X11FocusTransition::Clear { previous: None }
            }
            None => X11FocusTransition::Clear { previous: None },
        };
        self.route_authority_control(route, Some(transition))?;
        *focused = None;
        Ok(())
    }

    fn route_focus_out(
        &self,
        previous: XServerFrontendSurfaceRoute,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(previous.client)?.control;
        self.route_to_client(
            previous.client,
            sender,
            X11RoutedControl::FocusOut {
                window: previous.window,
            },
        )
    }

    fn route_authority_control(
        &self,
        route: XAuthorityClientControlCommand,
        focus: Option<X11FocusTransition>,
    ) -> Result<(), XServerFrontendRouteError> {
        let sender = self.client_senders(route.client)?.control;
        self.route_to_client(
            route.client,
            sender,
            X11RoutedControl::Authority {
                command: route.command,
                focus,
            },
        )
    }
}
