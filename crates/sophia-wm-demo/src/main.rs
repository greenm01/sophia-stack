fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("policy-v1-proof") {
        let socket = args.get(1).ok_or("missing policy socket")?;
        let cycles = args
            .get(2)
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(1);
        if !(1..=16).contains(&cycles) || args.len() > 3 {
            return Err("usage: sophia-wm-demo policy-v1-proof SOCKET [CYCLES]".into());
        }
        let mut client =
            sophia_wm_demo::PolicyV1Client::connect(socket, std::time::Duration::from_secs(2))?;
        for _ in 0..cycles {
            let snapshot = client.receive_snapshot()?;
            let request = client.receive_projection_request()?;
            let proposal = client.tile_once(&snapshot.scene, &request)?;
            client.send_projection(&proposal)?;
            let _ = client.receive_projection_outcome(&proposal)?;
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("serve-socket") {
        let socket = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--socket="))
            .ok_or("missing --socket=PATH")?;
        let wm_config = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--wm-config="))
            .map(std::path::Path::new);
        let no_wm_config = args.iter().any(|arg| arg == "--no-wm-config");
        sophia_wm_demo::run_socket_server_with_config_observer(
            socket,
            wm_config,
            no_wm_config,
            |event| println!("{event}"),
        )?;
        return Ok(());
    }

    let response = sophia_wm_demo::run_process_request(&args)?;
    print!("{response}");
    Ok(())
}
