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
        let (output_start_sender, output_start_receiver) = std::sync::mpsc::sync_channel(1);
        let _output_thread = std::thread::Builder::new()
            .name("sophia-output-v1-reference".to_owned())
            .spawn(move || {
                let result = (|| -> Result<_, sophia_wm_demo::OutputV1ClientError> {
                    output_start_receiver
                        .recv_timeout(std::time::Duration::from_secs(10))
                        .map_err(|_| {
                            sophia_wm_demo::OutputV1ClientError::InvalidProofTopology(
                                "proof surfaces did not become ready",
                            )
                        })?;
                    let mut client = sophia_wm_demo::OutputV1Client::connect(
                        output_socket,
                        std::time::Duration::from_secs(8),
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
        let mut output_started = false;
        let mut output_settled = false;
        loop {
            if output_started && !output_settled {
                // Collect the topology outcome without ever ceasing to service
                // the policy connection. Blocking here starves the owner's
                // writes: it keeps issuing cycles while the topology applies,
                // and a socket this side has stopped draining fills until the
                // owner's write deadline fires and it restarts this process
                // mid-apply.
                match output_result_receiver.try_recv() {
                    Ok(result) => {
                        let outcome = result?;
                        debug_assert_eq!(
                            outcome.kind,
                            sophia_protocol::OutputV1OutcomeKind::Committed
                        );
                        output_settled = true;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Err("output reference client ended without an outcome".into());
                    }
                }
            }
            let snapshot = match policy.receive_snapshot() {
                Ok(snapshot) => snapshot,
                // The owner issues no cycle while a topology transaction
                // prepares. That quiet window is expected, not a fault, so keep
                // waiting for whichever result arrives first.
                Err(error)
                    if output_started
                        && !output_settled
                        && sophia_wm_demo::policy_client_read_timed_out(&error) =>
                {
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            let request = policy.receive_projection_request()?;
            // Once the physical role has formed two logical groups, partition
            // proof surfaces through policy-visible geometry alone. Connector
            // labels used by the output role never enter blind policy.
            let mut scene = snapshot.scene;
            let partitioned = scene.outputs.len() == 2
                && scene
                    .outputs
                    .iter()
                    .all(|output| request.affected_outputs.contains(&output.output));
            if partitioned {
                let mut relocated = scene.clone();
                sophia_wm_demo::partition_policy_scene_across_outputs(&mut relocated)?;
                scene = relocated;
            }
            let proposal = policy.tile_once(&scene, &request)?;
            policy.send_projection(&proposal)?;
            let projection_outcome = policy.receive_projection_outcome(&proposal)?;
            match sophia_wm_demo::stateless_reference_projection_decision(projection_outcome) {
                sophia_wm_demo::StatelessReferenceProjectionDecision::Settled => {}
                sophia_wm_demo::StatelessReferenceProjectionDecision::RetryFreshSnapshot => {
                    // This bundled proof policy is a pure projection of the
                    // received snapshot. It has no speculative private model
                    // to discard when Engine advances the scene generation.
                    continue;
                }
                sophia_wm_demo::StatelessReferenceProjectionDecision::Fatal => {
                    return Err(format!(
                        "policy reference projection did not commit: {projection_outcome:?}"
                    )
                    .into());
                }
            }
            let proof_surfaces_placed =
                sophia_wm_demo::policy_projection_distinct_surface_count(&proposal) >= 2;
            if !output_started && proof_surfaces_placed {
                output_start_sender
                    .send(())
                    .map_err(|_| "output reference client start barrier disconnected")?;
                output_started = true;
            }
            if output_settled && partitioned {
                let placement_counts = proposal
                    .outputs
                    .iter()
                    .map(|output| output.placements.len())
                    .collect::<Vec<_>>();
                if placement_counts != [1, 1] {
                    return Err(format!(
                        "mixed proof did not place one surface on each logical output: {placement_counts:?}"
                    )
                    .into());
                }
                println!(
                    "sophia_wm_v1_reference schema=1 status=settled outputs=2 surfaces=2 placement=1,1"
                );
                // The proof policy is now stable. Leave the process alive for
                // visual inspection without creating no-op projection traffic.
                loop {
                    std::thread::park();
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
