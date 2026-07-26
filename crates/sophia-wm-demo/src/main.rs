fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
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
