fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("live-mixed-output-proof") {
        if args.len() != 4 {
            return Err(
                "usage: sophia-wm-demo live-mixed-output-proof MIRROR_PRIMARY MIRROR_MEMBER EXTENDED"
                    .into(),
            );
        }
        let policy_socket = std::env::var("SOPHIA_WM_SOCKET")?;
        let output_socket = std::env::var("SOPHIA_OUTPUT_SOCKET")?;
        let labels = [args[1].clone(), args[2].clone(), args[3].clone()];
        let (output_result_sender, output_result_receiver) = std::sync::mpsc::sync_channel(1);
        let _output_thread = std::thread::Builder::new()
            .name("sophia-output-v1-reference".to_owned())
            .spawn(move || {
                let result = (|| -> Result<_, sophia_wm_demo::OutputV1ClientError> {
                    let mut client = sophia_wm_demo::OutputV1Client::connect(
                        output_socket,
                        std::time::Duration::from_secs(4),
                    )?;
                    let (_, snapshot) = client.receive_snapshot()?;
                    let candidate = sophia_wm_demo::mixed_mirror_extended_candidate(
                        &snapshot, &labels[0], &labels[1], &labels[2],
                    )?;
                    let outcome = client.submit(candidate, &snapshot)?;
                    if outcome.kind != sophia_protocol::OutputV1OutcomeKind::Committed {
                        return Err(sophia_wm_demo::OutputV1ClientError::NonCommittedOutcome(
                            outcome.kind,
                        ));
                    }
                    println!(
                        "sophia_output_v1_reference schema=1 status=settled kind={:?} topology_epoch={} heads=3 groups=2",
                        outcome.kind, outcome.topology_epoch,
                    );
                    Ok(outcome)
                })();
                let _ = output_result_sender.send(result);
            })?;
        let mut policy = sophia_wm_demo::PolicyV1Client::connect(
            policy_socket,
            std::time::Duration::from_secs(4),
        )?;
        policy.activate_profile_and_configure()?;
        let mut output_settled = false;
        loop {
            let snapshot = policy.receive_snapshot()?;
            let request = policy.receive_projection_request()?;
            // Once the physical role has formed two logical groups, partition
            // proof surfaces through policy-visible geometry alone. Connector
            // labels used by the output role never enter blind policy.
            let mut scene = snapshot.scene;
            if scene.outputs.len() == 2 {
                let mut relocated = scene.clone();
                let rightmost =
                    sophia_wm_demo::partition_policy_scene_across_outputs(&mut relocated)?;
                if request.affected_outputs.contains(&rightmost) {
                    scene = relocated;
                }
            }
            let proposal = policy.tile_once(&scene, &request)?;
            policy.send_projection(&proposal)?;
            let _ = policy.receive_projection_outcome(&proposal)?;
            if !output_settled {
                match output_result_receiver.try_recv() {
                    Ok(Ok(outcome)) => {
                        debug_assert_eq!(
                            outcome.kind,
                            sophia_protocol::OutputV1OutcomeKind::Committed
                        );
                        output_settled = true;
                    }
                    Ok(Err(error)) => return Err(error.into()),
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Err("output reference client disconnected without a result".into());
                    }
                }
            }
        }
    }
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
