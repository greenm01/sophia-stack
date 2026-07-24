fn window_table_with_surface(window: XResourceId, namespace: NamespaceId) -> XWindowTable {
    let mut windows = XWindowTable::new();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: window,
            surface: SurfaceId::new(3, 1),
            namespace,
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
        })
        .unwrap();
    windows
}

fn window_table_with_two_surfaces(
    first: XResourceId,
    first_namespace: NamespaceId,
    second: XResourceId,
    second_namespace: NamespaceId,
) -> XWindowTable {
    let mut windows = XWindowTable::new();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: first,
            surface: SurfaceId::new(4, 1),
            namespace: first_namespace,
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
        })
        .unwrap();
    windows
        .apply(XWindowLifecycleEvent::Created {
            id: second,
            surface: SurfaceId::new(5, 1),
            namespace: second_namespace,
            geometry: Rect {
                x: 660,
                y: 20,
                width: 640,
                height: 480,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        })
        .unwrap();
    windows
}

fn create_window_request(
    transaction: TransactionId,
    namespace: NamespaceId,
) -> XAuthorityRequestPacket {
    XAuthorityRequestPacket {
        transaction,
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window: XResourceId::new(0xc0, 1),
            surface: SurfaceId::new(30, 1),
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
    }
}

fn create_second_window_request(
    transaction: TransactionId,
    namespace: NamespaceId,
) -> XAuthorityRequestPacket {
    XAuthorityRequestPacket {
        transaction,
        namespace,
        kind: XAuthorityRequestKind::CreateWindow {
            window: XResourceId::new(0xc1, 1),
            surface: SurfaceId::new(31, 1),
            geometry: Rect {
                x: 700,
                y: 20,
                width: 320,
                height: 240,
            },
            constraints: SurfaceConstraints {
                min_size: None,
                max_size: None,
            },
            generation: 1,
        },
    }
}

#[cfg(unix)]
fn wait_for_socket(path: &std::path::Path) {
    for _ in 0..100 {
        if path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out waiting for socket {}", path.display());
}
