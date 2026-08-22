use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sophia_protocol::{BrokerV1Request, BrokerV1Response, SurfaceId, TransactionId};
use sophia_runtime::{
    MetadataBrokerClientTransport, MetadataBrokerSessionTransport, PolicyPeerIdentity,
};

#[test]
fn an_idle_broker_connection_outlives_the_request_timeout() {
    let directory = std::env::temp_dir().join(format!(
        "sophia-broker-idle-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut session = MetadataBrokerSessionTransport::bind(
        &directory,
        PolicyPeerIdentity {
            uid: rustix::process::geteuid().as_raw(),
            pid: std::process::id(),
        },
    )
    .unwrap();
    let socket = session.socket_path().to_path_buf();
    let client = std::thread::spawn(move || {
        let mut client = MetadataBrokerClientTransport::connect(socket).unwrap();
        let (transaction, request) = client.receive().unwrap();
        assert!(matches!(request, BrokerV1Request::SurfaceRemoved { .. }));
        client
            .respond(
                transaction,
                &BrokerV1Response::NoChange {
                    connection_epoch: 1,
                },
            )
            .unwrap();
    });
    session
        .accept_and_negotiate(1, Duration::from_secs(2))
        .unwrap();

    // Session requests have a five-second reply bound. An idle role server must
    // not inherit that read timeout or a quiet desktop kills it before this send.
    std::thread::sleep(Duration::from_millis(5_100));
    assert_eq!(
        session
            .request(
                TransactionId::from_raw(1),
                &BrokerV1Request::SurfaceRemoved {
                    connection_epoch: 1,
                    surface: SurfaceId::new(1, 1),
                },
            )
            .unwrap(),
        BrokerV1Response::NoChange {
            connection_epoch: 1
        }
    );
    session.disconnect().unwrap();
    client.join().unwrap();
}
