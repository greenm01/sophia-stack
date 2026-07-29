#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::{
    collections::VecDeque,
    fmt,
    io::{ErrorKind, Read, Write},
    path::Path,
    time::Duration,
};

#[cfg(unix)]
use sophia_protocol::{
    IpcMessageKind, SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, WM_API_VERSION, WmActionId,
    WmBindingRegistration, WmCapabilities, WmChromePolicy, WmFocusRingStyle, WmFrameStyle, WmHello,
    WmModifierMask, WmPolicyAck, WmPolicyAckOutcome, WmPolicyUpdate, WmRequestPacket, WmRgb8,
    decode_frame, decode_wm_policy_ack_frame, decode_wm_request_frame,
    decode_wm_session_descriptor_frame, encode_wm_hello_frame, encode_wm_policy_update_frame,
    encode_wm_response_frame,
};

#[cfg(unix)]
use crate::{WmProcessError, handle_wm_request_with_config};

#[cfg(unix)]
const WM_SOCKET_IDLE_POLL: Duration = Duration::from_millis(25);
#[cfg(unix)]
const WM_SOCKET_FRAME_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WmConfigReloadEvent {
    Ready {
        generation: u64,
        layout: sophia_config::WmLayoutKind,
    },
    Candidate {
        generation: u64,
    },
    Applied {
        generation: u64,
    },
    Unchanged {
        generation: u64,
    },
    Rejected {
        stage: &'static str,
        message: String,
    },
}

impl fmt::Display for WmConfigReloadEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready { generation, layout } => write!(
                formatter,
                "sophia_wm_demo schema=1 status=ready generation={generation} layout_policy={}",
                layout.name()
            ),
            Self::Candidate { generation } => write!(
                formatter,
                "sophia_wm_config_reload schema=2 status=candidate generation={generation}"
            ),
            Self::Applied { generation } => write!(
                formatter,
                "sophia_wm_config_reload schema=2 status=applied generation={generation}"
            ),
            Self::Unchanged { generation } => write!(
                formatter,
                "sophia_wm_config_reload schema=2 status=unchanged generation={generation}"
            ),
            Self::Rejected { stage, message } => write!(
                formatter,
                "sophia_wm_config_reload schema=2 status=rejected reason={stage} error={message}"
            ),
        }
    }
}

pub fn run_socket_server(path: impl AsRef<Path>) -> Result<(), WmProcessError> {
    run_socket_server_with_config(path, None, false)
}

pub fn run_socket_server_with_config(
    path: impl AsRef<Path>,
    explicit_config: Option<&Path>,
    no_config: bool,
) -> Result<(), WmProcessError> {
    run_socket_server_with_config_observer(path, explicit_config, no_config, |_| {})
}

pub fn run_socket_server_with_config_observer(
    path: impl AsRef<Path>,
    explicit_config: Option<&Path>,
    no_config: bool,
    mut observe: impl FnMut(WmConfigReloadEvent),
) -> Result<(), WmProcessError> {
    if no_config && explicit_config.is_some() {
        return Err(WmProcessError::new(
            "--no-wm-config and --wm-config are mutually exclusive",
        ));
    }
    let source = if no_config {
        sophia_config::ConfigSource {
            class: sophia_config::ConfigSourceClass::CompiledDefault,
            path: None,
        }
    } else {
        sophia_config::discover_default_config_source(
            sophia_config::ConfigDomain::Wm,
            explicit_config,
        )
    };
    let mut config = sophia_config::WmConfigState::load(&source)
        .map_err(|error| WmProcessError::new(format!("failed to load WM config: {error}")))?;
    observe(WmConfigReloadEvent::Ready {
        generation: config.active().generation.raw(),
        layout: config.active().layout,
    });
    let source_path = source.path.clone();
    let watcher = source
        .path
        .as_deref()
        .map(sophia_config::ConfigWatcher::spawn)
        .transpose()
        .map_err(|error| WmProcessError::new(format!("failed to watch WM config: {error}")))?;
    let path = path.as_ref();
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(WmProcessError::new(format!(
                "failed to remove stale socket {}: {error}",
                path.display()
            )));
        }
    }

    let listener = UnixListener::bind(path).map_err(|error| {
        WmProcessError::new(format!(
            "failed to bind WM socket {}: {error}",
            path.display()
        ))
    })?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => serve_socket_client(
                &mut stream,
                &mut config,
                watcher.as_ref(),
                source_path.as_deref(),
                &mut observe,
            )?,
            Err(error) => {
                return Err(WmProcessError::new(format!(
                    "failed to accept WM socket client on {}: {error}",
                    path.display()
                )));
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn serve_socket_client(
    stream: &mut UnixStream,
    config: &mut sophia_config::WmConfigState,
    watcher: Option<&sophia_config::ConfigWatcher>,
    source_path: Option<&Path>,
    observe: &mut dyn FnMut(WmConfigReloadEvent),
) -> Result<(), WmProcessError> {
    service_wm_config(config, watcher, source_path, observe);
    let snapshot = config.active();
    let policy = wm_policy_update(snapshot);
    let hello = WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        policy_generation: policy.generation,
        bindings: policy.bindings,
        chrome: policy.chrome,
    };
    let frame = encode_wm_hello_frame(&hello)
        .map_err(|error| WmProcessError::new(format!("failed to encode WM hello: {error:?}")))?;
    stream
        .write_all(&frame)
        .and_then(|()| stream.flush())
        .map_err(|error| WmProcessError::new(format!("failed to write WM hello: {error}")))?;
    let descriptor = read_frame(stream)?;
    decode_wm_session_descriptor_frame(&descriptor).map_err(|error| {
        WmProcessError::new(format!("failed to decode WM session descriptor: {error:?}"))
    })?;

    let mut acknowledged_generation = config.active().generation.raw();
    let mut deferred_requests = VecDeque::with_capacity(1);
    loop {
        service_wm_config(config, watcher, source_path, observe);
        if config.active().generation.raw() > acknowledged_generation {
            let update = wm_policy_update(config.active());
            write_policy_update(stream, &update)?;
            let exchange = await_policy_acknowledgement(stream)?;
            let acknowledgement = exchange.acknowledgement;
            if acknowledgement.generation != update.generation {
                return Err(WmProcessError::new(format!(
                    "WM policy acknowledgement generation mismatch: expected {}, got {}",
                    update.generation, acknowledgement.generation
                )));
            }
            if acknowledgement.outcome != WmPolicyAckOutcome::Applied {
                return Err(WmProcessError::new(format!(
                    "engine rejected WM policy generation {} with outcome {:?}",
                    update.generation, acknowledgement.outcome
                )));
            }
            acknowledged_generation = update.generation;
            observe(WmConfigReloadEvent::Applied {
                generation: update.generation,
            });
            if let Some(request) = exchange.deferred_request {
                deferred_requests.push_back(request);
            }
        }

        let input = match deferred_requests.pop_front() {
            Some(request) => WmSocketInput::Request(request),
            None => read_wm_socket_input(stream)?,
        };
        match input {
            WmSocketInput::Request(request) => {
                write_wm_response(stream, request, config.active())?;
            }
            WmSocketInput::Idle => {}
            WmSocketInput::Closed => break,
        }
    }

    Ok(())
}

#[cfg(unix)]
fn wm_policy_update(snapshot: &sophia_config::WmConfigSnapshot) -> WmPolicyUpdate {
    WmPolicyUpdate {
        api_version: WM_API_VERSION,
        generation: snapshot.generation.raw(),
        bindings: snapshot
            .bindings
            .iter()
            .map(|binding| WmBindingRegistration {
                action: WmActionId::from_raw(binding.action),
                keycode: binding.keycode,
                modifiers: WmModifierMask {
                    bits: binding.modifiers,
                },
            })
            .collect(),
        chrome: WmChromePolicy {
            focus_ring: WmFocusRingStyle {
                enabled: snapshot.chrome.focus_ring.enabled,
                width: snapshot.chrome.focus_ring.width,
                color: WmRgb8 {
                    red: snapshot.chrome.focus_ring.color.red,
                    green: snapshot.chrome.focus_ring.color.green,
                    blue: snapshot.chrome.focus_ring.color.blue,
                },
            },
            frame: WmFrameStyle {
                enabled: snapshot.chrome.frame.enabled,
                width: snapshot.chrome.frame.width,
                focused_color: WmRgb8 {
                    red: snapshot.chrome.frame.focused_color.red,
                    green: snapshot.chrome.frame.focused_color.green,
                    blue: snapshot.chrome.frame.focused_color.blue,
                },
                unfocused_color: WmRgb8 {
                    red: snapshot.chrome.frame.unfocused_color.red,
                    green: snapshot.chrome.frame.unfocused_color.green,
                    blue: snapshot.chrome.frame.unfocused_color.blue,
                },
            },
        },
    }
}

#[cfg(unix)]
fn service_wm_config(
    config: &mut sophia_config::WmConfigState,
    watcher: Option<&sophia_config::ConfigWatcher>,
    source_path: Option<&Path>,
    observe: &mut dyn FnMut(WmConfigReloadEvent),
) {
    let mut changed = false;
    if let Some(watcher) = watcher {
        while watcher.try_recv().is_ok() {
            changed = true;
        }
    }
    if !changed {
        return;
    }
    let Some(path) = source_path else {
        return;
    };
    match sophia_config::read_config_file(path) {
        Ok(bytes) => match config.reload(&bytes) {
            Ok(report) if report.disposition == sophia_config::ReloadDisposition::Applied => {
                observe(WmConfigReloadEvent::Candidate {
                    generation: report.generation.raw(),
                });
            }
            Ok(report) => {
                observe(WmConfigReloadEvent::Unchanged {
                    generation: report.generation.raw(),
                });
            }
            Err(error) => {
                observe(WmConfigReloadEvent::Rejected {
                    stage: "parse",
                    message: error.to_string(),
                });
            }
        },
        Err(error) => {
            observe(WmConfigReloadEvent::Rejected {
                stage: "read",
                message: error.to_string(),
            });
        }
    }
}

#[cfg(unix)]
enum WmSocketInput {
    Request(WmRequestPacket),
    Idle,
    Closed,
}

#[cfg(unix)]
fn read_wm_socket_input(stream: &mut UnixStream) -> Result<WmSocketInput, WmProcessError> {
    stream
        .set_read_timeout(Some(WM_SOCKET_IDLE_POLL))
        .map_err(|error| {
            WmProcessError::new(format!("failed to set WM socket timeout: {error}"))
        })?;
    let mut ready = [0u8; 1];
    match rustix::net::recv(&*stream, &mut ready, rustix::net::RecvFlags::PEEK) {
        Ok((0, _)) => return Ok(WmSocketInput::Closed),
        Ok(_) => {}
        Err(error) => {
            let error = std::io::Error::from(error);
            if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                return Ok(WmSocketInput::Idle);
            }
            return Err(WmProcessError::new(format!(
                "failed to poll WM socket: {error}"
            )));
        }
    }

    stream
        .set_read_timeout(Some(WM_SOCKET_FRAME_TIMEOUT))
        .map_err(|error| WmProcessError::new(format!("failed to set WM frame timeout: {error}")))?;
    let frame = read_frame(stream)?;
    decode_wm_request_frame(&frame)
        .map(WmSocketInput::Request)
        .map_err(|error| WmProcessError::new(format!("failed to decode WM request: {error:?}")))
}

#[cfg(unix)]
fn write_policy_update(
    stream: &mut UnixStream,
    update: &WmPolicyUpdate,
) -> Result<(), WmProcessError> {
    let frame = encode_wm_policy_update_frame(update).map_err(|error| {
        WmProcessError::new(format!("failed to encode WM policy update: {error:?}"))
    })?;
    stream
        .write_all(&frame)
        .and_then(|()| stream.flush())
        .map_err(|error| WmProcessError::new(format!("failed to write WM policy update: {error}")))
}

#[cfg(unix)]
struct WmPolicyExchange {
    acknowledgement: WmPolicyAck,
    deferred_request: Option<WmRequestPacket>,
}

#[cfg(unix)]
fn await_policy_acknowledgement(
    stream: &mut UnixStream,
) -> Result<WmPolicyExchange, WmProcessError> {
    stream
        .set_read_timeout(None)
        .map_err(|error| WmProcessError::new(format!("failed to set WM frame timeout: {error}")))?;
    let mut deferred_request = None;
    loop {
        let frame = read_frame(stream)?;
        let (header, _) = decode_frame(&frame).map_err(|error| {
            WmProcessError::new(format!(
                "failed to decode message during WM policy exchange: {error:?}"
            ))
        })?;
        match header.message_kind {
            IpcMessageKind::WmPolicyAck => {
                let acknowledgement = decode_wm_policy_ack_frame(&frame).map_err(|error| {
                    WmProcessError::new(format!(
                        "failed to decode WM policy acknowledgement: {error:?}"
                    ))
                })?;
                return Ok(WmPolicyExchange {
                    acknowledgement,
                    deferred_request,
                });
            }
            IpcMessageKind::WmRequest if deferred_request.is_none() => {
                deferred_request = Some(decode_wm_request_frame(&frame).map_err(|error| {
                    WmProcessError::new(format!("failed to decode deferred WM request: {error:?}"))
                })?);
            }
            IpcMessageKind::WmRequest => {
                return Err(WmProcessError::new(
                    "more than one WM request arrived during policy exchange",
                ));
            }
            _ => {
                return Err(WmProcessError::new(
                    "unexpected message during WM policy exchange",
                ));
            }
        }
    }
}

#[cfg(unix)]
fn write_wm_response(
    stream: &mut UnixStream,
    request: WmRequestPacket,
    config: &sophia_config::WmConfigSnapshot,
) -> Result<(), WmProcessError> {
    let response = handle_wm_request_with_config(request, config);
    let frame = encode_wm_response_frame(&response)
        .map_err(|error| WmProcessError::new(format!("failed to encode WM response: {error:?}")))?;
    stream
        .write_all(&frame)
        .and_then(|()| stream.flush())
        .map_err(|error| WmProcessError::new(format!("failed to write WM response: {error}")))
}

fn read_frame(stream: &mut UnixStream) -> Result<Vec<u8>, WmProcessError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    stream
        .read_exact(&mut header)
        .map_err(|error| WmProcessError::new(format!("failed to read IPC header: {error}")))?;
    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmProcessError::new(format!(
            "IPC payload too large: {payload_len}"
        )));
    }
    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| WmProcessError::new(format!("failed to read IPC payload: {error}")))?;
    Ok(frame)
}
