#[cfg(unix)]
fn x11_focus_event_record(
    byte_order: XByteOrder,
    sequence: u16,
    window: XResourceId,
    focused: bool,
) -> Vec<u8> {
    encode_x_client_event(
        byte_order,
        XClientEvent::Focus {
            sequence,
            focused,
            detail: 3,
            event: window,
            mode: 0,
        },
    )
}

#[cfg(unix)]
fn x11_focus_surface_records(
    byte_order: XByteOrder,
    sequence: u16,
    window: XResourceId,
    previous_authority: XResourceId,
    previous_routed: XResourceId,
    transition: Option<X11FocusTransition>,
) -> Result<Vec<Vec<u8>>, X11SetupSocketError> {
    let transition = transition.unwrap_or_else(|| {
        if previous_authority == window && previous_routed == window {
            X11FocusTransition::Unchanged
        } else {
            X11FocusTransition::Enter {
                previous: (previous_routed != window
                    && previous_routed.local.raw() != u64::from(X_SETUP_DEFAULT_ROOT))
                .then_some(previous_routed),
            }
        }
    });
    match transition {
        X11FocusTransition::Unchanged => Ok(Vec::new()),
        X11FocusTransition::Enter { previous } => {
            let mut records = Vec::with_capacity(2);
            if let Some(previous) = previous {
                records.push(x11_focus_event_record(
                    byte_order,
                    sequence,
                    previous,
                    false,
                ));
            }
            records.push(x11_focus_event_record(byte_order, sequence, window, true));
            Ok(records)
        }
        X11FocusTransition::Clear { .. } => Err(X11SetupSocketError::new(
            "X11 routed focus transition mismatched FocusSurface",
        )),
    }
}

#[cfg(unix)]
fn x11_clear_focus_records(
    byte_order: XByteOrder,
    sequence: u16,
    root: XResourceId,
    previous_routed: XResourceId,
    transition: Option<X11FocusTransition>,
) -> Result<Vec<Vec<u8>>, X11SetupSocketError> {
    let previous = match transition {
        Some(X11FocusTransition::Clear { previous }) => previous,
        None if previous_routed != root => Some(previous_routed),
        None => None,
        Some(_) => {
            return Err(X11SetupSocketError::new(
                "X11 routed focus transition mismatched ClearFocus",
            ));
        }
    };
    Ok(previous
        .map(|previous| vec![x11_focus_event_record(byte_order, sequence, previous, false)])
        .unwrap_or_default())
}

#[cfg(unix)]
fn write_x11_control_records(
    stream: &Arc<Mutex<UnixStream>>,
    byte_order: XByteOrder,
    sequence: &AtomicU16,
    records: Vec<Vec<u8>>,
) -> Result<(), X11SetupSocketError> {
    let mut stream = stream
        .lock()
        .map_err(|_| X11SetupSocketError::new("X11 output socket lock poisoned"))?;
    let event_sequence = sequence.load(Ordering::Acquire);
    for mut record in records {
        write_xi_u16(byte_order, &mut record[2..4], event_sequence);
        if std::env::var_os("SOPHIA_X11_AUTHORITY_TRACE").is_some() {
            tracing::trace!(
                "sophia_x11_socket_write schema=1 writer=control bytes={} payload_redacted=true",
                record.len(),
            );
        }
        if let Err(error) = stream.write_all(&record) {
            if is_x11_client_disconnect(&error) {
                return Ok(());
            }
            return Err(X11SetupSocketError::new(format!(
                "failed to write X11 control event: {error}"
            )));
        }
    }
    stream.flush().map_err(|error| {
        X11SetupSocketError::new(format!("failed to flush X11 control event: {error}"))
    })
}
