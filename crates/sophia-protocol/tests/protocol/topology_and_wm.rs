#[test]
fn output_topology_validates_bounded_engine_facts() {
    let topology = OutputTopologySnapshot {
        generation: 7,
        primary: OutputId::from_raw(2),
        outputs: vec![
            OutputTopologyEntry {
                output: OutputId::from_raw(1),
                logical: Rect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
                pixel_size: Size {
                    width: 1920,
                    height: 1080,
                },
                scale: 1,
                refresh_millihz: 60_000,
                timing: None,
            },
            OutputTopologyEntry {
                output: OutputId::from_raw(2),
                logical: Rect {
                    x: 1920,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                pixel_size: Size {
                    width: 2560,
                    height: 1440,
                },
                scale: 2,
                refresh_millihz: 120_000,
                timing: None,
            },
        ],
    };
    assert_eq!(
        topology.validate(),
        Ok(Size {
            width: 3200,
            height: 1080,
        })
    );
}

#[test]
fn output_topology_rejects_duplicate_and_unbounded_facts() {
    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs.push(topology.outputs[0]);
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::DuplicateOutput)
    );

    let mut topology = OutputTopologySnapshot::deterministic();
    topology.outputs[0].logical.width = i32::from(u16::MAX) + 1;
    assert_eq!(
        topology.validate(),
        Err(OutputTopologyError::RootSizeExceeded)
    );
}
