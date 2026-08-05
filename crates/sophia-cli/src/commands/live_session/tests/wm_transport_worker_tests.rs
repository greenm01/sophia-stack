use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use sophia_engine::{WmSocketTransport, WmSocketTransportConfig};
use sophia_protocol::{
    OutputId, TransactionId, WmRelayoutWorkspace, WmRequestKind, WmRequestPacket, WorkspaceId,
};

use super::super::wm_transport_worker::WmTransportWorker;

#[test]
fn unanswered_request_observes_the_aggregate_transport_deadline() {
    let (client, _server) = UnixStream::pair().unwrap();
    let response_timeout = Duration::from_millis(30);
    let worker = WmTransportWorker::new(WmSocketTransport::new(
        client,
        WmSocketTransportConfig { response_timeout },
    ))
    .unwrap();
    let transaction = TransactionId::from_raw(7);
    worker
        .try_submit(WmRequestPacket {
            transaction,
            kind: WmRequestKind::RelayoutWorkspace(WmRelayoutWorkspace {
                output: OutputId::from_raw(1),
                workspace: WorkspaceId::from_raw(1),
                bounds: sophia_protocol::Rect {
                    x: 0,
                    y: 0,
                    width: 1280,
                    height: 720,
                },
                nodes: Vec::new(),
            }),
        })
        .unwrap();

    let test_deadline = Instant::now() + Duration::from_secs(1);
    let completion = loop {
        if let Some(completion) = worker.try_complete().unwrap() {
            break completion;
        }
        assert!(
            Instant::now() < test_deadline,
            "WM worker failed to enforce its aggregate response deadline"
        );
        std::thread::sleep(Duration::from_millis(2));
    };

    assert_eq!(completion.transaction, transaction);
    assert_eq!(completion.result.unwrap_err(), "WM response timed out");
    assert!(completion.elapsed >= response_timeout);
}
