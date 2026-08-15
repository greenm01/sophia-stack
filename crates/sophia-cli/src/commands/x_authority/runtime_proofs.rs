fn run_x_authority_present_pixmap_smoke()
-> Result<XAuthorityPresentPixmapSmokeReport, Box<dyn std::error::Error>> {
    let artifacts = run_x_authority_present_pixmap_smoke_artifacts()?;
    let runtime_state = runtime_state_from_observed_batches(&artifacts.batches)?;

    Ok(XAuthorityPresentPixmapSmokeReport {
        display: artifacts.display,
        extension_opcode: artifacts.extension_opcode,
        transactions: artifacts
            .batches
            .iter()
            .map(|batch| batch.transactions.len())
            .sum(),
        runtime_committed: runtime_state.authority_transactions_committed,
        runtime_surfaces: runtime_state.authority_surfaces_applied,
    })
}

#[cfg(feature = "atomic-scanout-live")]
pub(crate) fn collect_x_authority_present_pixmap_authority_batches()
-> Result<Vec<AuthorityTransactionIntake>, Box<dyn std::error::Error>> {
    let artifacts = run_x_authority_present_pixmap_smoke_artifacts()?;
    Ok(authority_intakes_from_observed_batches(&artifacts.batches))
}

#[derive(Clone, Debug)]
struct XAuthorityPresentPixmapSmokeArtifacts {
    display: String,
    extension_opcode: u8,
    batches: Vec<XAuthorityObservedTransactionBatch>,
}

fn run_x_authority_present_pixmap_smoke_artifacts()
-> Result<XAuthorityPresentPixmapSmokeArtifacts, Box<dyn std::error::Error>> {
    use std::io::Write;

    let (display, socket_path) = temp_xauthority_display(5600)?;
    let server_path = socket_path.clone();
    let (sender, receiver) = sync_channel(8);
    let server = std::thread::spawn(move || {
        run_x11_core_socket_server_once_channel(&server_path, NamespaceId::from_raw(47), sender)
    });

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    stream.write_all(&x11_setup_request(XByteOrder::LittleEndian))?;
    read_x11_setup_success(&mut stream, XByteOrder::LittleEndian)?;

    stream.write_all(&x11_query_extension_request(
        XByteOrder::LittleEndian,
        X_SOPHIA_PRESENT_EXTENSION_NAME,
    ))?;
    let extension = read_x11_record(&mut stream)?;
    if extension[8] != 1 || extension[9] != X_SOPHIA_PRESENT_MAJOR_OPCODE {
        return Err(format!(
            "SOPHIA-PRESENT query returned present={} opcode={}",
            extension[8], extension[9]
        )
        .into());
    }

    stream.write_all(&x11_create_window_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        20,
        30,
        640,
        480,
    ))?;
    let configure = read_x11_record(&mut stream)?;
    if configure[0] != 22 {
        return Err(format!("expected ConfigureNotify, got record {}", configure[0]).into());
    }

    stream.write_all(&x11_sophia_present_pixmap_request(
        XByteOrder::LittleEndian,
        0x0020_0001,
        0x0000_0990,
        (0, 0, 640, 480),
        1,
        250,
    ))?;

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority X11 socket server thread panicked")?
        .map_err(|error| format!("X authority X11 socket server failed: {error}"))?;
    let batches = receiver.try_iter().collect::<Vec<_>>();

    Ok(XAuthorityPresentPixmapSmokeArtifacts {
        display,
        extension_opcode: extension[9],
        batches,
    })
}

fn runtime_state_from_observed_batches(
    batches: &[XAuthorityObservedTransactionBatch],
) -> Result<sophia_runtime::SessionRuntimeState, Box<dyn std::error::Error>> {
    let transactions = batches
        .iter()
        .flat_map(|batch| batch.transactions.iter().cloned())
        .collect::<Vec<_>>();
    let engine = HeadlessEngine::default();
    let committed = seed_committed_states_for_transactions(&transactions);
    let (sender, receiver) = sync_channel(batches.len().max(1));
    for batch in authority_intakes_from_observed_batches(batches) {
        sender.try_send(batch)?;
    }
    let inbox = AuthorityTransactionInbox::new(receiver, batches.len().max(1));
    let mut assembly = HeadlessCompositorBackendAssembly::new(engine.output())
        .with_committed_surfaces(committed)
        .with_authority_inbox(inbox);
    let report = assembly.run_tick(CompositorBackendTickInput {
        x_event_count: u32::try_from(transactions.len()).unwrap_or(u32::MAX),
        authority_commits: Vec::new(),
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layer_templates: layer_templates_from_surface_transactions(&transactions),
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    })?;
    Ok(report.runtime.runtime_state)
}

fn authority_intakes_from_observed_batches(
    batches: &[XAuthorityObservedTransactionBatch],
) -> Vec<AuthorityTransactionIntake> {
    batches
        .iter()
        .map(|batch| {
            AuthorityTransactionIntake::new(batch.transaction, batch.transactions.clone())
                .with_surface_removals(batch.removed_surfaces.clone())
        })
        .collect()
}

#[cfg(feature = "atomic-scanout-live")]
fn authority_intakes_from_observed_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<AuthorityTransactionIntake> {
    transactions
        .iter()
        .map(|transaction| {
            AuthorityTransactionIntake::new(transaction.transaction, vec![transaction.clone()])
        })
        .collect()
}

fn runtime_state_from_observed_transactions(
    transactions: &[SurfaceTransaction],
) -> Result<sophia_runtime::SessionRuntimeState, Box<dyn std::error::Error>> {
    let engine = HeadlessEngine::default();
    let output = engine.output();
    let mut committed = seed_committed_states_for_transactions(transactions);
    let mut commits = Vec::new();

    for transaction in transactions {
        commits.push(engine.commit_surface_transactions(
            transaction.transaction,
            std::slice::from_ref(transaction),
            &mut committed,
        ));
    }

    let mut driver = HeadlessSessionDriver::new(engine);
    let mut adapter = LiveRuntimeDriverAdapter::from_intake(LiveRuntimeDriverIntake {
        x_event_count: u32::try_from(transactions.len()).unwrap_or(u32::MAX),
        authority_commits: commits,
        authority_batches: Vec::new(),
        wm_update: None,
        portal_commands: Vec::new(),
        chrome_command_count: 0,
        layers: layer_templates_from_surface_transactions(transactions),
        committed_surfaces: committed,
        scanout_submit_state: None,
        scanout_lifecycle_states: Vec::new(),
    });
    let report = driver.run_with_adapter(output.id, 1, &mut adapter)?;
    Ok(report.runtime_state)
}

fn seed_committed_states_for_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<CommittedSurfaceState> {
    let mut surfaces = std::collections::BTreeMap::new();
    for transaction in transactions {
        surfaces
            .entry(transaction.surface)
            .or_insert(CommittedSurfaceState {
                surface: transaction.surface,
                committed_generation: transaction.previous_committed_generation,
                geometry: transaction.target_geometry,
                content: transaction.content.clone(),
                damage: Region::empty(),
            });
    }
    surfaces.into_values().collect()
}

pub(crate) fn layer_templates_from_surface_transactions(
    transactions: &[SurfaceTransaction],
) -> Vec<LayerSnapshot> {
    transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| LayerSnapshot {
            surface: transaction.surface,
            authority_local_id: None,
            namespace: None,
            stack_rank: u32::try_from(index).unwrap_or(u32::MAX),
            geometry: transaction.target_geometry,
            source: BufferSource::None,
            damage: transaction.damage.clone(),
            opacity: 1.0,
            crop: None,
            transform: Transform::IDENTITY,
            generation: transaction.previous_committed_generation,
            resize_sync: ResizeSyncCapability::ImplicitOnly,
        })
        .collect()
}

fn run_x_authority_runtime_smoke()
-> Result<XAuthorityRuntimeSmokeReport, Box<dyn std::error::Error>> {
    let socket_path = std::env::temp_dir().join(format!(
        "sophia-x-authority-runtime-{}-{}.sock",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let server_path = socket_path.clone();
    let server = std::thread::spawn(move || run_x_authority_socket_server_once(&server_path));

    wait_for_socket_path(&socket_path)?;
    let mut stream = UnixStream::connect(&socket_path)?;
    let trusted = NamespaceId::from_raw(31);
    let untrusted = NamespaceId::from_raw(32);

    let create_source = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(301),
            namespace: trusted,
            kind: XAuthorityRequestKind::CreateWindow {
                window: XResourceId::new(0xd0, 1),
                surface: SurfaceId::new(301, 1),
                geometry: Rect {
                    x: 10,
                    y: 20,
                    width: 640,
                    height: 480,
                },
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        },
    )?;
    let create_target = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(302),
            namespace: untrusted,
            kind: XAuthorityRequestKind::CreateWindow {
                window: XResourceId::new(0xd1, 1),
                surface: SurfaceId::new(302, 1),
                geometry: Rect {
                    x: 700,
                    y: 20,
                    width: 480,
                    height: 360,
                },
                constraints: SurfaceConstraints {
                    min_size: None,
                    max_size: None,
                },
                generation: 1,
            },
        },
    )?;
    let present = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(303),
            namespace: trusted,
            kind: XAuthorityRequestKind::PresentPixmap {
                window: XResourceId::new(0xd0, 1),
                pixmap: 0x990,
                damage: Region::single(Rect {
                    x: 0,
                    y: 0,
                    width: 640,
                    height: 480,
                }),
                previous_committed_generation: 1,
                timeout_msec: 250,
            },
        },
    )?;
    let _selection_owner = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(304),
            namespace: trusted,
            kind: XAuthorityRequestKind::SetSelectionOwner {
                selection: 1,
                owner: Some(XResourceId::new(0xd0, 1)),
                timestamp: 10,
                selection_timestamp: 10,
                kind: XAuthoritySelectionChangeKind::SetOwner,
            },
        },
    )?;
    let selection = send_request(
        &mut stream,
        XAuthorityRequestPacket {
            transaction: TransactionId::from_raw(305),
            namespace: untrusted,
            kind: XAuthorityRequestKind::RequestSelection {
                requestor: XResourceId::new(0xd1, 1),
                selection: 1,
                target: 2,
                target_name: "UTF8_STRING".to_owned(),
                property: 3,
                time: 11,
                transfer: PortalTransferId::from_raw(401),
            },
        },
    )?;

    let surfaces = create_source.surfaces.len() + create_target.surfaces.len();
    let transactions = present.transactions.len();
    let portal_prompts = selection.portal_commands.len();
    let selection_artifacts = selection.selection_artifacts.len();

    drop(stream);
    let _ = std::fs::remove_file(&socket_path);
    server
        .join()
        .map_err(|_| "X authority socket server thread panicked")??;

    Ok(XAuthorityRuntimeSmokeReport {
        socket_path,
        surfaces,
        transactions,
        portal_prompts,
        selection_artifacts,
    })
}

