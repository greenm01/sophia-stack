use std::num::NonZeroUsize;
use std::thread;

use sophia_protocol::{
    ClientAdmissionContext, ClientAdmissionId, ClientAuthProvenance, ClientAuthenticationMethod,
    NamespaceCapabilities, NamespaceContext, NamespaceId, NamespaceProfile,
};
use sophia_x_authority::{
    XAuthorityExplicitPointerGrabAnchor, XAuthorityExplicitPointerGrabBridgeError,
    XAuthorityExplicitPointerGrabRequestKind, XAuthorityExplicitPointerGrabResponse,
    x_authority_explicit_pointer_grab_bridge,
};

fn admission() -> ClientAdmissionContext {
    ClientAdmissionContext::new(
        ClientAdmissionId::from_raw(4),
        NamespaceContext::new(
            NamespaceId::from_raw(3),
            NamespaceProfile::Confined,
            NamespaceCapabilities::NONE,
        )
        .unwrap(),
        ClientAuthProvenance::new(ClientAuthenticationMethod::PeerCredentials, 9).unwrap(),
    )
    .unwrap()
}

#[test]
fn explicit_pointer_grab_bridge_rejects_queue_saturation() {
    let (client, owner) = x_authority_explicit_pointer_grab_bridge(NonZeroUsize::new(1).unwrap());
    let first_client = client.clone();
    let worker = thread::spawn(move || {
        first_client.request(
            admission(),
            XAuthorityExplicitPointerGrabRequestKind::Prepare {
                anchor: XAuthorityExplicitPointerGrabAnchor::AdmissionDefault,
                replaces: None,
            },
        )
    });
    while owner.pending() == 0 {
        thread::yield_now();
    }

    assert_eq!(
        client.request(
            admission(),
            XAuthorityExplicitPointerGrabRequestKind::Prepare {
                anchor: XAuthorityExplicitPointerGrabAnchor::AdmissionDefault,
                replaces: None,
            },
        ),
        Err(XAuthorityExplicitPointerGrabBridgeError::Capacity),
    );

    let request = owner.try_recv().unwrap();
    owner
        .respond(
            request.id,
            XAuthorityExplicitPointerGrabResponse::Rejected(
                sophia_x_authority::XAuthorityExplicitPointerGrabRejection::AlreadyOwned,
            ),
        )
        .unwrap();
    assert!(worker.join().unwrap().is_ok());
}

#[test]
fn explicit_pointer_grab_bridge_fails_closed_when_owner_disconnects() {
    let (client, owner) = x_authority_explicit_pointer_grab_bridge(NonZeroUsize::new(1).unwrap());
    drop(owner);

    assert_eq!(
        client.request(
            admission(),
            XAuthorityExplicitPointerGrabRequestKind::Prepare {
                anchor: XAuthorityExplicitPointerGrabAnchor::AdmissionDefault,
                replaces: None,
            },
        ),
        Err(XAuthorityExplicitPointerGrabBridgeError::Disconnected),
    );
}

#[test]
fn explicit_pointer_grab_bridge_correlates_bounded_passive_records() {
    let (client, owner) = x_authority_explicit_pointer_grab_bridge(NonZeroUsize::new(2).unwrap());
    let worker = thread::spawn(move || {
        client.request(
            admission(),
            XAuthorityExplicitPointerGrabRequestKind::Prepare {
                anchor: XAuthorityExplicitPointerGrabAnchor::AdmissionDefault,
                replaces: None,
            },
        )
    });
    let request = loop {
        if let Ok(request) = owner.try_recv() {
            break request;
        }
        thread::yield_now();
    };
    assert_eq!(request.admission, admission());
    owner
        .respond(
            request.id,
            XAuthorityExplicitPointerGrabResponse::Rejected(
                sophia_x_authority::XAuthorityExplicitPointerGrabRejection::NotViewable,
            ),
        )
        .unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap(),
        XAuthorityExplicitPointerGrabResponse::Rejected(
            sophia_x_authority::XAuthorityExplicitPointerGrabRejection::NotViewable,
        )
    );
}
