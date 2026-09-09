use std::{
    net::SocketAddr,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use terminal_poker::{
    game_stream::{self, GameStream},
    network_server::{MultiTableNetworkServer, MultiTableNetworkServerConfig},
    network_transport::{
        read_available, write_message, ClientWireMessage, FrameDecoder, ServerWireMessage,
        WIRE_VERSION,
    },
};
struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

fn receive(stream: &mut GameStream, decoder: &mut FrameDecoder) -> ServerWireMessage {
    let started = Instant::now();
    loop {
        read_available(stream, decoder).unwrap();
        if let Some(message) = decoder.decode_next().unwrap() {
            return message;
        }
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn waiting_polls_do_not_share_admission_budget_and_rejection_closes_tls_cleanly() {
    use terminal_poker::lobby::{LobbyEnvelope, LobbyRequest};
    let server = Server::start("server.pem");
    let mut clients = Vec::new();
    for _ in 0..4 {
        let mut stream =
            GameStream::connect_tls(server.address, include_bytes!("fixtures/tls/ca.der")).unwrap();
        let mut decoder = FrameDecoder::default();
        write_message(
            &mut stream,
            &ClientWireMessage::Connect {
                version: WIRE_VERSION,
                label: "waiting regression".into(),
                reconnect: None,
            },
        )
        .unwrap();
        assert!(matches!(
            receive(&mut stream, &mut decoder),
            ServerWireMessage::LobbyWelcome { .. }
        ));
        clients.push((stream, decoder));
    }
    // Four clients behind one IP, ordinary two-per-second polling, across a
    // complete limiter window. No registration exists: each status must return
    // its ordinary lobby error, never a transport disconnect or rate rejection.
    for id in 0..24 {
        for (stream, decoder) in &mut clients {
            write_message(
                stream,
                &ClientWireMessage::Lobby {
                    request: LobbyEnvelope::new(format!("status-{id}"), LobbyRequest::JoinStatus),
                },
            )
            .unwrap();
            assert!(matches!(
                receive(stream, decoder),
                ServerWireMessage::LobbyError { .. }
            ));
        }
        thread::sleep(Duration::from_millis(500));
    }
    // Actual excess still rejects; the framed reason survives the TLS shutdown.
    let (stream, decoder) = &mut clients[0];
    let mut rejected = false;
    for id in 0..61 {
        write_message(
            stream,
            &ClientWireMessage::Lobby {
                request: LobbyEnvelope::new(format!("health-{id}"), LobbyRequest::Health),
            },
        )
        .unwrap();
        if let ServerWireMessage::Error { error } = receive(stream, decoder) {
            assert_eq!(error.code, "rate_limited");
            rejected = true;
            break;
        }
    }
    assert!(rejected);
    let started = Instant::now();
    while read_available(stream, decoder).unwrap()
        != terminal_poker::network_transport::ReadStatus::Closed
    {
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
}
impl Server {
    fn start(cert: &str) -> Self {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tls");
        let stop = Arc::new(AtomicBool::new(false));
        let server = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            tls: Some(
                game_stream::server_config(&fixtures.join(cert), &fixtures.join("server.key"))
                    .unwrap(),
            ),
            shutdown_requested: Arc::clone(&stop),
            ..Default::default()
        })
        .unwrap();
        let address = server.listen_addr();
        let worker = thread::spawn(move || {
            server.run().unwrap();
        });
        Self {
            address,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
    }
}

#[test]
fn tls_verifies_trust_name_and_expiry_before_lobby_credentials() {
    let ca = include_bytes!("fixtures/tls/ca.der");
    for certificate in ["wrong-name.pem", "expired.pem"] {
        let server = Server::start(certificate);
        assert!(
            GameStream::connect_tls(server.address, ca).is_err(),
            "must reject {certificate}"
        );
    }
    let server = Server::start("server.pem");
    assert!(
        GameStream::connect_tls(server.address, game_stream::DEPLOYMENT_CA).is_err(),
        "untrusted issuer accepted"
    );
    let mut stream = GameStream::connect_tls(server.address, ca).unwrap();
    write_message(
        &mut stream,
        &ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: "TLS test".into(),
            reconnect: None,
        },
    )
    .unwrap();
    let mut decoder = FrameDecoder::default();
    let started = Instant::now();
    loop {
        read_available(&mut stream, &mut decoder).unwrap();
        if let Some(message) = decoder.decode_next::<ServerWireMessage>().unwrap() {
            assert!(matches!(message, ServerWireMessage::LobbyWelcome { .. }));
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(3));
        thread::sleep(Duration::from_millis(2));
    }
    write_message(&mut stream, &ClientWireMessage::Close).unwrap();
}

#[test]
fn remote_plaintext_bind_is_rejected_and_tls_listener_rejects_plaintext() {
    assert!(
        MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            bind: "0.0.0.0:0".parse().unwrap(),
            ..Default::default()
        })
        .is_err()
    );
    let server = Server::start("server.pem");
    let mut socket = std::net::TcpStream::connect(server.address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write_message(
        &mut socket,
        &ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: "plaintext probe".into(),
            reconnect: None,
        },
    )
    .unwrap();
    let mut bytes = [0u8; 1024];
    use std::io::Read;
    match socket.read(&mut bytes) {
        Ok(0) | Err(_) => {}
        Ok(n) => assert!(!String::from_utf8_lossy(&bytes[..n]).contains("lobby_welcome")),
    }
}
