use std::{
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use sophia_protocol::{
    SOPHIA_IPC_HEADER_LEN, WM_API_VERSION, WmSessionDescriptor, encode_wm_session_descriptor_frame,
};
use sophia_x11_wm_bridge::{LegacyWmLaunchSpec, run_wm_socket_server};

#[test]
fn control_client_disconnect_ends_the_bridge_server() {
    let socket = unique_socket_path();
    let (done_tx, done_rx) = mpsc::channel();
    let server_socket = socket.clone();
    thread::spawn(move || {
        let result = run_wm_socket_server(&server_socket, LegacyWmLaunchSpec::new("/bin/false"));
        let _ = done_tx.send(result);
    });

    let deadline = Instant::now() + Duration::from_secs(1);
    let mut client = loop {
        match UnixStream::connect(&socket) {
            Ok(client) => break client,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            Err(error) => panic!("WM socket did not become ready: {error}"),
        }
    };
    let descriptor = encode_wm_session_descriptor_frame(&WmSessionDescriptor {
        api_version: WM_API_VERSION,
        workspaces: Vec::new(),
        active_workspaces: Vec::new(),
        session_actions: Vec::new(),
    })
    .unwrap();
    let mut hello_header = [0; SOPHIA_IPC_HEADER_LEN];
    client.read_exact(&mut hello_header).unwrap();
    let hello_payload_len = u32::from_le_bytes(hello_header[16..20].try_into().unwrap()) as usize;
    let mut hello_payload = vec![0; hello_payload_len];
    client.read_exact(&mut hello_payload).unwrap();
    client.write_all(&descriptor).unwrap();
    drop(client);

    let result = done_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("bridge server retained its runtime after the control client disconnected");
    result.unwrap();
    assert!(!socket.exists());
}

fn unique_socket_path() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "sophia-wm-server-lifecycle-{}-{}.sock",
        std::process::id(),
        thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_file(&path);
    path
}
