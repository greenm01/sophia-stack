#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::{
    io::{ErrorKind, Read, Write},
    path::Path,
};

#[cfg(unix)]
use sophia_protocol::{
    SOPHIA_IPC_HEADER_LEN, SOPHIA_IPC_MAX_PAYLOAD_LEN, WM_API_VERSION, WmActionId,
    WmBindingRegistration, WmCapabilities, WmChromeStyle, WmHello, WmModifierMask, WmRequestPacket,
    WmRgb8, decode_wm_request_frame, decode_wm_session_descriptor_frame, encode_wm_hello_frame,
    encode_wm_response_frame,
};

#[cfg(unix)]
use crate::{WmProcessError, handle_wm_request_with_config};

pub fn run_socket_server(path: impl AsRef<Path>) -> Result<(), WmProcessError> {
    run_socket_server_with_config(path, None, false)
}

pub fn run_socket_server_with_config(
    path: impl AsRef<Path>,
    explicit_config: Option<&Path>,
    no_config: bool,
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
) -> Result<(), WmProcessError> {
    service_wm_config(config, watcher, source_path);
    let snapshot = config.active();
    let hello = WmHello {
        api_version: WM_API_VERSION,
        capabilities: WmCapabilities::all_supported(),
        policy_generation: snapshot.generation.raw(),
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
        chrome: WmChromeStyle {
            enabled: snapshot.chrome.enabled,
            thickness: snapshot.chrome.thickness,
            color: WmRgb8 {
                red: snapshot.chrome.color.red,
                green: snapshot.chrome.color.green,
                blue: snapshot.chrome.color.blue,
            },
        },
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

    while let Some(request) = read_wm_request(stream)? {
        service_wm_config(config, watcher, source_path);
        let response = handle_wm_request_with_config(request, config.active());
        let frame = encode_wm_response_frame(&response).map_err(|error| {
            WmProcessError::new(format!("failed to encode WM response: {error:?}"))
        })?;
        stream.write_all(&frame).map_err(|error| {
            WmProcessError::new(format!("failed to write WM response: {error}"))
        })?;
        stream.flush().map_err(|error| {
            WmProcessError::new(format!("failed to flush WM response: {error}"))
        })?;
    }

    Ok(())
}

#[cfg(unix)]
fn service_wm_config(
    config: &mut sophia_config::WmConfigState,
    watcher: Option<&sophia_config::ConfigWatcher>,
    source_path: Option<&Path>,
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
    if let Ok(bytes) = sophia_config::read_config_file(path) {
        let _ = config.reload(&bytes);
    }
}

#[cfg(unix)]
fn read_wm_request(stream: &mut UnixStream) -> Result<Option<WmRequestPacket>, WmProcessError> {
    let mut header = [0; SOPHIA_IPC_HEADER_LEN];
    match stream.read_exact(&mut header) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => {
            return Err(WmProcessError::new(format!(
                "failed to read WM request header: {error}"
            )));
        }
    }

    let payload_len = u32::from_le_bytes(
        header[16..20]
            .try_into()
            .expect("fixed IPC header payload range should be present"),
    ) as usize;
    if payload_len > SOPHIA_IPC_MAX_PAYLOAD_LEN {
        return Err(WmProcessError::new(format!(
            "WM request payload too large: {payload_len}"
        )));
    }

    let mut frame = Vec::with_capacity(SOPHIA_IPC_HEADER_LEN + payload_len);
    frame.extend_from_slice(&header);
    frame.resize(SOPHIA_IPC_HEADER_LEN + payload_len, 0);
    stream
        .read_exact(&mut frame[SOPHIA_IPC_HEADER_LEN..])
        .map_err(|error| {
            WmProcessError::new(format!("failed to read WM request payload: {error}"))
        })?;

    decode_wm_request_frame(&frame)
        .map(Some)
        .map_err(|error| WmProcessError::new(format!("failed to decode WM request: {error:?}")))
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
