use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const CTRL_BREAK_EVENT: u32 = 1;

#[cfg(windows)]
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn GenerateConsoleCtrlEvent(control_event: u32, process_group_id: u32) -> i32;
}

use serde::Deserialize;
use terminal_poker::lobby::PublicTableSummary;
use terminal_poker::network_session::{NetworkSession, NetworkSessionError};
use terminal_poker::network_transport::MAX_WIRE_FRAME_BYTES;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Deserialize)]
struct ClientSummary {
    session: String,
    table_id: u64,
    hand_id: u64,
    initial_revision: u64,
    initial_awards: usize,
    terminal_revision: u64,
    phase: String,
    reconnects: u64,
    controls_enabled: bool,
    chip_total: u32,
    server_errors: u64,
    old_credential_rejected: bool,
}

#[cfg(windows)]
#[test]
fn operating_system_interrupt_drains_once_restores_and_ignores_corrupt_history() {
    let checkpoint = std::env::temp_dir().join(format!(
        "terminal-poker-signal-checkpoint-{}.json",
        std::process::id()
    ));
    let history = checkpoint.with_extension("history.json");
    let _ = std::fs::remove_file(&checkpoint);
    let _ = std::fs::remove_file(&history);

    let (mut first, mut first_stdout, first_address) = start_signal_server(&checkpoint, &history);
    let created = create_table_process(&first_address, "signal-creator", "Signal Room");
    let signal_started = Instant::now();
    send_console_break(first.id());
    wait_success_with_timeout(&mut first, "signal drain server", Duration::from_secs(5));
    assert!(signal_started.elapsed() < Duration::from_secs(5));
    let mut first_remainder = String::new();
    first_stdout.read_to_string(&mut first_remainder).unwrap();
    assert!(
        first_remainder.contains("stop_reason=Interrupt"),
        "{first_remainder}"
    );
    assert!(
        first_remainder.contains("drain_checkpoint=true"),
        "{first_remainder}"
    );
    assert_eq!(first_remainder.matches("drain_checkpoint=true").count(), 1);
    assert!(checkpoint.exists());
    assert!(history.exists());

    let valid_history = std::fs::read_to_string(&history).unwrap();
    std::fs::write(
        &history,
        valid_history.replacen(
            "terminal-poker-safe-ring-history",
            "terminal-poker-corrupt-ring-history",
            1,
        ),
    )
    .unwrap();
    let (mut restored, mut restored_stdout, restored_address) =
        start_signal_server(&checkpoint, &history);
    let listed = client_output(&[
        "--connect",
        &restored_address,
        "--session",
        "signal-browser",
        "--lobby-list",
    ]);
    let tables: Vec<PublicTableSummary> = serde_json::from_str(listed.trim()).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].table_id, created.table_id);
    send_console_break(restored.id());
    wait_success_with_timeout(
        &mut restored,
        "restored signal drain server",
        Duration::from_secs(5),
    );
    let mut restored_remainder = String::new();
    restored_stdout
        .read_to_string(&mut restored_remainder)
        .unwrap();
    assert!(
        restored_remainder.contains("history_recovery=CorruptIgnored"),
        "{restored_remainder}"
    );
    assert!(restored_remainder.contains("stop_reason=Interrupt"));

    std::fs::remove_file(checkpoint).unwrap();
    std::fs::remove_file(history).unwrap();
}

#[test]
fn two_tables_restart_from_checkpoint_and_complete_fresh_process_hands() {
    let checkpoint = std::env::temp_dir().join(format!(
        "terminal-poker-process-restart-{}.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&checkpoint);
    let history = checkpoint.with_extension("history.json");
    let _ = std::fs::remove_file(&history);
    let credential_dir = std::env::temp_dir().join(format!(
        "terminal-poker-process-restart-credentials-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&credential_dir);
    let (mut first_server, mut first_stdout, first_address) =
        start_checkpoint_server(&checkpoint, &history);
    let alpha = create_table_process(&first_address, "restart-create-a", "Restart Alpha");
    let bravo = create_table_process(&first_address, "restart-create-b", "Restart Bravo");
    let first = run_two_table_clients(&first_address, &alpha, &bravo, true, &credential_dir);
    wait_success(&mut first_server, "pre-restart checkpoint server");
    let mut first_remainder = String::new();
    first_stdout
        .read_to_string(&mut first_remainder)
        .expect("read pre-restart server completion");
    assert!(first_remainder.contains("completed_hands=2"));
    assert!(first_remainder.contains("safe_histories=2"));
    assert!(checkpoint.exists());
    assert!(history.exists());

    let (mut second_server, mut second_stdout, second_address) =
        start_checkpoint_server(&checkpoint, &history);
    let listed = client_output(&[
        "--connect",
        &second_address,
        "--session",
        "restart-browser",
        "--lobby-list",
    ]);
    let restored_tables: Vec<PublicTableSummary> =
        serde_json::from_str(listed.trim()).expect("valid restored lobby list");
    assert_eq!(restored_tables.len(), 2);
    assert_eq!(restored_tables[0].table_id, alpha.table_id);
    assert_eq!(restored_tables[1].table_id, bravo.table_id);
    assert!(restored_tables.iter().all(|table| table.occupied == 2));
    let health = client_output(&[
        "--connect",
        &second_address,
        "--session",
        "restart-health",
        "--health",
    ]);
    let health: serde_json::Value = serde_json::from_str(health.trim()).unwrap();
    assert_eq!(health["healthy"], true);
    assert_eq!(health["tables"], 2);
    assert_eq!(health["routed_sessions"], 4);
    assert_eq!(health["checkpoint_version"], 4);
    assert_eq!(
        health["recovery_boundary"],
        "latest_validated_between_hand_checkpoint"
    );

    let second = run_two_table_clients(&second_address, &alpha, &bravo, false, &credential_dir);
    for restored in &second {
        let prior = first
            .iter()
            .find(|summary| summary.session == restored.session)
            .expect("same durable session completed before restart");
        assert!(restored.hand_id > prior.hand_id);
        assert_eq!(restored.table_id, prior.table_id);
        assert_eq!(restored.chip_total, prior.chip_total);
        assert_eq!(restored.initial_revision, 0);
        assert_eq!(restored.initial_awards, 0);
        assert_eq!(restored.server_errors, 0);
    }
    wait_success(&mut second_server, "post-restart checkpoint server");
    let mut second_remainder = String::new();
    second_stdout
        .read_to_string(&mut second_remainder)
        .expect("read post-restart server completion");
    assert!(second_remainder.contains("tables=2"));
    assert!(second_remainder.contains("completed_hands=2"));
    assert!(second_remainder.contains("history_recovery=Loaded"));
    assert!(second_remainder.contains("safe_histories=4"));
    std::fs::remove_file(checkpoint).expect("remove process checkpoint");
    std::fs::remove_file(history).expect("remove process history");
    std::fs::remove_dir_all(credential_dir).expect("remove process credential directory");
}

#[test]
fn independent_processes_complete_and_reconnect_at_every_occupancy() {
    for seats in 2u8..=9 {
        run_occupancy(seats);
    }
}

#[test]
fn production_server_rejects_duplicate_sessions_and_oversized_frames() {
    let (mut server, address) = start_open_server();

    let (mut first, _) = NetworkSession::connect(address, "player-s0").expect("first session");
    let duplicate = match NetworkSession::connect(address, "player-s0") {
        Ok(_) => panic!("duplicate active session unexpectedly connected"),
        Err(error) => error,
    };
    assert!(matches!(
        duplicate,
        NetworkSessionError::Rejected { ref code, .. } if code == "duplicate_active_session"
    ));
    first.close().expect("close first session");
    drop(first);

    let mut hostile = TcpStream::connect(address).expect("hostile peer connects");
    hostile
        .write_all(
            &u32::try_from(MAX_WIRE_FRAME_BYTES + 1)
                .unwrap()
                .to_be_bytes(),
        )
        .expect("hostile header reaches server");
    hostile.shutdown(Shutdown::Both).ok();
    thread::sleep(Duration::from_millis(30));

    let (mut valid, _) = NetworkSession::connect(address, "player-s1")
        .expect("server remains available after hostile frame");
    valid.close().expect("close valid session");
    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn expired_process_reconnect_credential_fails_closed() {
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args([
            "--multi-table",
            "--max-tables",
            "2",
            "--seed",
            "61500",
            "--reconnect-ttl-seconds",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("expiry server starts");
    let mut stdout = BufReader::new(server.stdout.take().unwrap());
    let mut listening = String::new();
    stdout.read_line(&mut listening).unwrap();
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap()
        .to_string();
    let table = create_table_process(&address, "expiry-create", "Expiry");
    let mut expiring = Command::new(env!("CARGO_BIN_EXE_poker-client"))
        .args([
            "--connect",
            &address,
            "--session",
            "expiry-a",
            "--join-table",
            &table.table_id.0.to_string(),
            "--seat",
            "0",
            "--headless",
            "--disconnect-after-revision",
            "1",
            "--disconnect-pause-ms",
            "1250",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut peer = Command::new(env!("CARGO_BIN_EXE_poker-client"))
        .args([
            "--connect",
            &address,
            "--session",
            "expiry-b",
            "--join-table",
            &table.table_id.0.to_string(),
            "--seat",
            "1",
            "--headless",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stderr = wait_failure_output(&mut expiring, Duration::from_secs(6));
    assert!(stderr.contains("reconnect_rejected"), "{stderr}");
    let _ = peer.kill();
    let _ = peer.wait();
    let _ = server.kill();
    let _ = server.wait();
}

#[test]
fn independent_processes_create_list_join_and_isolate_two_tables() {
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args([
            "--multi-table",
            "--max-tables",
            "4",
            "--seed",
            "10000",
            "--exit-after-hands",
            "2",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("multi-table server process starts");
    let mut server_stdout = BufReader::new(server.stdout.take().expect("server stdout pipe"));
    let mut listening = String::new();
    server_stdout
        .read_line(&mut listening)
        .expect("multi-table server announces address");
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("unexpected multi-table announcement: {listening}"));

    let alpha = create_table_process(address, "creator-alpha", "Alpha");
    let bravo = create_table_process(address, "creator-bravo", "Bravo");
    assert_ne!(alpha.table_id, bravo.table_id);
    assert_eq!(alpha.occupied, 0);
    assert_eq!(bravo.occupied, 0);

    let listed = client_output(&[
        "--connect",
        address,
        "--session",
        "browser-1",
        "--lobby-list",
    ]);
    let tables: Vec<PublicTableSummary> =
        serde_json::from_str(listed.trim()).expect("valid bounded public lobby JSON");
    assert_eq!(tables, vec![alpha.clone(), bravo.clone()]);

    let mut clients = Vec::new();
    for (table, prefix) in [(alpha.table_id.0, "alpha"), (bravo.table_id.0, "bravo")] {
        for seat in 0..2u8 {
            let mut command = Command::new(env!("CARGO_BIN_EXE_poker-client"));
            command.args([
                "--connect",
                address,
                "--session",
                &format!("{prefix}-s{seat}"),
                "--join-table",
                &table.to_string(),
                "--seat",
                &seat.to_string(),
                "--headless",
            ]);
            if prefix == "alpha" && seat == 1 {
                command.args([
                    "--disconnect-after-revision",
                    "2",
                    "--probe-rotated-credential",
                ]);
            }
            if prefix == "bravo" && seat == 0 {
                command.args(["--probe-wrong-table", &alpha.table_id.0.to_string()]);
            }
            clients.push(
                command
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("routed client starts"),
            );
        }
    }

    let mut summaries = Vec::new();
    for (index, client) in clients.iter_mut().enumerate() {
        wait_success(client, &format!("multi-table client {index}"));
        let mut stdout = String::new();
        client
            .stdout
            .take()
            .expect("client stdout pipe")
            .read_to_string(&mut stdout)
            .expect("read routed client summary");
        summaries.push(
            serde_json::from_str::<ClientSummary>(stdout.trim())
                .expect("valid routed client summary"),
        );
    }
    assert_eq!(summaries.len(), 4);
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.table_id == alpha.table_id.0)
            .count(),
        2
    );
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.table_id == bravo.table_id.0)
            .count(),
        2
    );
    for summary in &summaries {
        assert!(summary.terminal_revision > 0);
        assert!(matches!(summary.phase.as_str(), "Showdown" | "Complete"));
        assert_eq!(summary.chip_total, 200);
        assert!(!summary.controls_enabled);
    }
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.reconnects == 1)
            .count(),
        1
    );
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.old_credential_rejected)
            .count(),
        1
    );
    assert_eq!(
        summaries
            .iter()
            .filter(|summary| summary.server_errors == 1)
            .count(),
        1
    );
    assert!(summaries.iter().all(|summary| summary.server_errors <= 1));

    wait_success(&mut server, "multi-table server");
    let mut remainder = String::new();
    server_stdout
        .read_to_string(&mut remainder)
        .expect("read multi-table server completion");
    assert!(remainder.contains("mode=multi"), "{remainder}");
    assert!(remainder.contains("tables=2"), "{remainder}");
    assert!(remainder.contains("completed_hands=2"), "{remainder}");
}

#[test]
fn private_table_process_is_hidden_and_fails_closed_before_authorized_play() {
    let code = "private-process-code-0123456789abcdef";
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args([
            "--multi-table",
            "--max-tables",
            "2",
            "--seed",
            "51000",
            "--exit-after-hands",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("private candidate server starts");
    let mut server_stdout = BufReader::new(server.stdout.take().unwrap());
    let mut listening = String::new();
    server_stdout.read_line(&mut listening).unwrap();
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap();

    let created = client_output(&[
        "--connect",
        address,
        "--session",
        "private-create",
        "--create-table",
        "Invite Only",
        "--table-seats",
        "2",
        "--min-players",
        "2",
        "--table-stack",
        "100",
        "--table-visibility",
        "private",
        "--join-code",
        code,
    ]);
    let private: PublicTableSummary = serde_json::from_str(created.trim()).unwrap();
    let listed = client_output(&[
        "--connect",
        address,
        "--session",
        "private-browser",
        "--lobby-list",
    ]);
    assert_eq!(
        serde_json::from_str::<Vec<PublicTableSummary>>(listed.trim()).unwrap(),
        vec![]
    );
    let denied = client_failure(&[
        "--connect",
        address,
        "--session",
        "private-denied",
        "--join-table",
        &private.table_id.0.to_string(),
        "--headless",
    ]);
    assert!(denied.contains("unknown_table"), "{denied}");
    assert!(!denied.contains("Invite Only"));
    assert!(!denied.contains(code));

    let mut clients = (0..2u8)
        .map(|seat| {
            Command::new(env!("CARGO_BIN_EXE_poker-client"))
                .args([
                    "--connect",
                    address,
                    "--session",
                    &format!("private-s{seat}"),
                    "--join-table",
                    &private.table_id.0.to_string(),
                    "--seat",
                    &seat.to_string(),
                    "--join-code",
                    code,
                    "--headless",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect::<Vec<_>>();
    for (index, client) in clients.iter_mut().enumerate() {
        wait_success(client, &format!("private client {index}"));
        let mut stdout = String::new();
        client
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        let summary: ClientSummary = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(summary.table_id, private.table_id.0);
        assert_eq!(summary.chip_total, 200);
        assert_eq!(summary.server_errors, 0);
    }
    wait_success(&mut server, "private candidate server");
}

#[test]
#[ignore = "explicit private-beta 8-table/32-session capacity profile"]
fn private_beta_capacity_profile_completes_eight_tables_and_mass_reconnects() {
    let started = Instant::now();
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args(["--multi-table", "--max-tables", "8", "--seed", "42000"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("capacity server starts");
    let mut server_stdout = BufReader::new(server.stdout.take().expect("server stdout"));
    let mut listening = String::new();
    server_stdout
        .read_line(&mut listening)
        .expect("listen line");
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("unexpected server announcement: {listening}"));

    let tables = (0..8)
        .map(|index| {
            create_table_process_with_min(
                address,
                &format!("load-create-{index}"),
                &format!("Load {index}"),
                4,
            )
        })
        .collect::<Vec<_>>();
    let mut clients = Vec::with_capacity(32);
    for (table_index, table) in tables.iter().enumerate() {
        for seat in 0..4u8 {
            clients.push(
                Command::new(env!("CARGO_BIN_EXE_poker-client"))
                    .args([
                        "--connect",
                        address,
                        "--session",
                        &format!("load-t{table_index}-s{seat}"),
                        "--join-table",
                        &table.table_id.0.to_string(),
                        "--seat",
                        &seat.to_string(),
                        "--headless",
                        "--disconnect-after-revision",
                        "2",
                    ])
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("capacity client starts"),
            );
        }
    }

    let mut summaries = Vec::with_capacity(32);
    for (index, client) in clients.iter_mut().enumerate() {
        wait_success_with_timeout(
            client,
            &format!("capacity client {index}"),
            Duration::from_secs(45),
        );
        let mut stdout = String::new();
        client
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        summaries
            .push(serde_json::from_str::<ClientSummary>(stdout.trim()).expect("capacity summary"));
    }
    assert_eq!(summaries.len(), 32);
    assert!(summaries.iter().all(|summary| summary.reconnects == 1));
    assert!(summaries.iter().all(|summary| summary.server_errors == 0));
    assert!(summaries.iter().all(|summary| summary.chip_total == 400));
    for table in &tables {
        assert_eq!(
            summaries
                .iter()
                .filter(|summary| summary.table_id == table.table_id.0)
                .count(),
            4
        );
    }
    let health_json =
        client_output(&["--connect", address, "--session", "load-health", "--health"]);
    let health: serde_json::Value = serde_json::from_str(health_json.trim()).unwrap();
    assert_eq!(health["tables"], 8);
    assert_eq!(health["routed_sessions"], 32);
    assert_eq!(health["healthy"], true);
    server.kill().ok();
    server.wait().ok();
    assert!(
        started.elapsed() <= Duration::from_secs(60),
        "capacity profile exceeded 60 seconds: {:?}",
        started.elapsed()
    );
}

fn run_occupancy(seats: u8) {
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args([
            "--seats",
            &seats.to_string(),
            "--stack",
            "100",
            "--seed",
            &format!("80{seats}"),
            "--exit-after-hand",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server process starts");
    let mut server_stdout = BufReader::new(server.stdout.take().expect("server stdout pipe"));
    let mut listening = String::new();
    server_stdout
        .read_line(&mut listening)
        .expect("server announces its address");
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("unexpected server announcement: {listening}"));

    let mut clients = (0..seats)
        .map(|seat| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_poker-client"));
            command.args([
                "--connect",
                address,
                "--session",
                &format!("player-s{seat}"),
                "--headless",
            ]);
            if seat == 1 {
                command.args(["--disconnect-after-revision", "2"]);
            }
            command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("client process starts")
        })
        .collect::<Vec<_>>();

    for (seat, client) in clients.iter_mut().enumerate() {
        wait_success(client, &format!("{seats}-seat client S{seat}"));
        let mut stdout = String::new();
        client
            .stdout
            .take()
            .expect("client stdout pipe")
            .read_to_string(&mut stdout)
            .expect("read client summary");
        let summary: ClientSummary =
            serde_json::from_str(stdout.trim()).expect("valid client summary JSON");
        assert_eq!(summary.session, format!("player-s{seat}"));
        assert_eq!(summary.table_id, 1);
        assert!(summary.terminal_revision > 0);
        assert!(matches!(summary.phase.as_str(), "Showdown" | "Complete"));
        assert_eq!(summary.reconnects, u64::from(seat == 1));
        assert!(!summary.controls_enabled);
        assert_eq!(summary.chip_total, u32::from(seats) * 100);
        assert_eq!(summary.server_errors, 0);
    }

    wait_success(&mut server, &format!("{seats}-seat server"));
    let mut remainder = String::new();
    server_stdout
        .read_to_string(&mut remainder)
        .expect("read server completion");
    assert!(remainder.contains("COMPLETE address="), "{remainder}");
    let accepted = remainder
        .split("accepted_connections=")
        .nth(1)
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("accepted connection metric");
    assert!(accepted >= u64::from(seats + 1), "{remainder}");
}

fn start_checkpoint_server(
    checkpoint: &Path,
    history: &Path,
) -> (Child, BufReader<ChildStdout>, String) {
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args([
            "--multi-table",
            "--max-tables",
            "4",
            "--seed",
            "12000",
            "--exit-after-hands",
            "2",
            "--checkpoint",
        ])
        .arg(checkpoint)
        .arg("--history")
        .arg(history)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("checkpoint server starts");
    let mut stdout = BufReader::new(server.stdout.take().expect("server stdout pipe"));
    let mut listening = String::new();
    stdout
        .read_line(&mut listening)
        .expect("checkpoint server announces address");
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("unexpected checkpoint announcement: {listening}"))
        .to_string();
    (server, stdout, address)
}

#[cfg(windows)]
fn start_signal_server(
    checkpoint: &Path,
    history: &Path,
) -> (Child, BufReader<ChildStdout>, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_poker-server"));
    command
        .args(["--multi-table", "--max-tables", "4", "--seed", "73000"])
        .arg("--checkpoint")
        .arg(checkpoint)
        .arg("--history")
        .arg(history)
        .creation_flags(CREATE_NEW_PROCESS_GROUP)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut server = command.spawn().expect("signal server starts");
    let mut stdout = BufReader::new(server.stdout.take().expect("signal server stdout"));
    let mut listening = String::new();
    stdout.read_line(&mut listening).unwrap();
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .unwrap_or_else(|| panic!("unexpected signal announcement: {listening}"))
        .to_string();
    (server, stdout, address)
}

#[cfg(windows)]
fn send_console_break(process_group_id: u32) {
    // SAFETY: the child was created as a new process group and the call takes
    // only value parameters. A zero result is reported as an OS failure.
    let delivered = unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, process_group_id) };
    assert_ne!(delivered, 0, "GenerateConsoleCtrlEvent failed");
}

fn run_two_table_clients(
    address: &str,
    alpha: &PublicTableSummary,
    bravo: &PublicTableSummary,
    join: bool,
    credential_dir: &Path,
) -> Vec<ClientSummary> {
    let mut clients = Vec::new();
    for (table, prefix) in [
        (alpha.table_id.0, "restart-a"),
        (bravo.table_id.0, "restart-b"),
    ] {
        for seat in 0..2u8 {
            let mut command = Command::new(env!("CARGO_BIN_EXE_poker-client"));
            command.args([
                "--connect",
                address,
                "--session",
                &format!("{prefix}-s{seat}"),
                "--headless",
            ]);
            command
                .arg("--credential-file")
                .arg(credential_dir.join(format!("{prefix}-s{seat}.token")));
            if join {
                command.args([
                    "--join-table",
                    &table.to_string(),
                    "--seat",
                    &seat.to_string(),
                ]);
            }
            clients.push(
                command
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .expect("restart journey client starts"),
            );
        }
    }
    let mut summaries = Vec::new();
    for (index, client) in clients.iter_mut().enumerate() {
        wait_success(client, &format!("restart journey client {index}"));
        let mut stdout = String::new();
        client
            .stdout
            .take()
            .expect("restart client stdout")
            .read_to_string(&mut stdout)
            .expect("read restart summary");
        summaries.push(serde_json::from_str(stdout.trim()).expect("valid restart client summary"));
    }
    summaries
}

fn create_table_process(address: &str, session: &str, name: &str) -> PublicTableSummary {
    create_table_process_with_min(address, session, name, 2)
}

fn create_table_process_with_min(
    address: &str,
    session: &str,
    name: &str,
    min_players: u8,
) -> PublicTableSummary {
    let output = client_output(&[
        "--connect",
        address,
        "--session",
        session,
        "--create-table",
        name,
        "--table-seats",
        &min_players.to_string(),
        "--table-stack",
        "100",
        "--min-players",
        &min_players.to_string(),
    ]);
    serde_json::from_str(output.trim()).expect("valid created table summary")
}

fn client_output(arguments: &[&str]) -> String {
    let mut client = Command::new(env!("CARGO_BIN_EXE_poker-client"))
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("one-shot lobby client starts");
    wait_success(&mut client, "one-shot lobby client");
    let mut stdout = String::new();
    client
        .stdout
        .take()
        .expect("lobby stdout pipe")
        .read_to_string(&mut stdout)
        .expect("read lobby client output");
    stdout
}

fn client_failure(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_poker-client"))
        .args(arguments)
        .output()
        .expect("rejected lobby client runs");
    assert!(!output.status.success());
    String::from_utf8(output.stderr).expect("rejection is UTF-8")
}

fn start_open_server() -> (Child, SocketAddr) {
    let mut server = Command::new(env!("CARGO_BIN_EXE_poker-server"))
        .args(["--seats", "2", "--stack", "100", "--seed", "900"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("server process starts");
    let mut output = BufReader::new(server.stdout.take().expect("server stdout pipe"));
    let mut listening = String::new();
    output
        .read_line(&mut listening)
        .expect("server announces its address");
    let address = listening
        .strip_prefix("LISTENING ")
        .and_then(|line| line.split_whitespace().next())
        .expect("listening address")
        .parse()
        .expect("socket address");
    (server, address)
}

fn wait_success(child: &mut Child, label: &str) {
    wait_success_with_timeout(child, label, PROCESS_TIMEOUT);
}

fn wait_success_with_timeout(child: &mut Child, label: &str, timeout: Duration) {
    let started = Instant::now();
    loop {
        match child.try_wait().expect("poll child") {
            Some(status) => {
                if !status.success() {
                    let mut stderr = String::new();
                    child
                        .stderr
                        .take()
                        .expect("child stderr pipe")
                        .read_to_string(&mut stderr)
                        .expect("read child stderr");
                    panic!("{label} failed with {status}: {stderr}");
                }
                return;
            }
            None if started.elapsed() < timeout => {
                thread::sleep(Duration::from_millis(5));
            }
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{label} exceeded {timeout:?}");
            }
        }
    }
}

fn wait_failure_output(child: &mut Child, timeout: Duration) -> String {
    let started = Instant::now();
    loop {
        match child.try_wait().expect("poll expected failure") {
            Some(status) => {
                assert!(!status.success(), "child unexpectedly succeeded");
                let mut stderr = String::new();
                child
                    .stderr
                    .take()
                    .expect("failure stderr")
                    .read_to_string(&mut stderr)
                    .unwrap();
                return stderr;
            }
            None if started.elapsed() < timeout => thread::sleep(Duration::from_millis(5)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("expected failure exceeded {timeout:?}");
            }
        }
    }
}
