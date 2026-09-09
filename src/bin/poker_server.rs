use std::error::Error;
use std::io::{self, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use terminal_poker::game::seat::TableSize;
use terminal_poker::network_server::{session_id_for_seat, NetworkServer, NetworkServerConfig};
use terminal_poker::network_server::{MultiTableNetworkServer, MultiTableNetworkServerConfig};
use terminal_poker::table_registry::{
    DEFAULT_TABLE_REGISTRY_CAPACITY, MAX_TABLE_REGISTRY_CAPACITY,
};

#[derive(Debug, Parser)]
#[command(
    name = "poker-server",
    about = "Loopback multiplayer poker table authority",
    version
)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:0")]
    bind: SocketAddr,
    #[arg(long, requires_all = ["tls_key", "multi_table"])]
    tls_cert: Option<PathBuf>,
    #[arg(long, requires_all = ["tls_cert", "multi_table"])]
    tls_key: Option<PathBuf>,
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(2..=9))]
    seats: u8,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
    stack: u32,
    #[arg(long)]
    seed: Option<u64>,
    #[arg(long)]
    exit_after_hand: bool,
    #[arg(long)]
    multi_table: bool,
    #[arg(long, default_value_t = DEFAULT_TABLE_REGISTRY_CAPACITY as u8, value_parser = clap::value_parser!(u8).range(1..=MAX_TABLE_REGISTRY_CAPACITY as i64))]
    max_tables: u8,
    #[arg(long, default_value_t = 0)]
    exit_after_hands: usize,
    #[arg(long, requires = "multi_table")]
    checkpoint: Option<PathBuf>,
    #[arg(long, requires = "multi_table")]
    history: Option<PathBuf>,
    #[arg(long, default_value_t = 900, requires = "multi_table")]
    table_idle_seconds: u64,
    #[arg(long, default_value_t = 300, requires = "multi_table", value_parser = clap::value_parser!(u64).range(1..=3600))]
    reconnect_ttl_seconds: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.multi_table {
        let shutdown_requested = Arc::new(AtomicBool::new(false));
        let signal_flag = Arc::clone(&shutdown_requested);
        ctrlc::set_handler(move || {
            signal_flag.store(true, Ordering::Release);
        })?;
        let tls = match (&args.tls_cert, &args.tls_key) {
            (Some(cert), Some(key)) => Some(terminal_poker::game_stream::server_config(cert, key)?),
            _ => None,
        };
        let server = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            tls,
            bind: args.bind,
            max_tables: usize::from(args.max_tables),
            deterministic_seed_base: args.seed,
            exit_after_hands: args.exit_after_hands,
            checkpoint_path: args.checkpoint,
            history_path: args.history,
            table_idle_ttl: Duration::from_secs(args.table_idle_seconds),
            shutdown_requested,
            reconnect_credential_ttl: Duration::from_secs(args.reconnect_ttl_seconds),
        })?;
        println!(
            "LISTENING {} mode=multi max_tables={}",
            server.listen_addr(),
            args.max_tables
        );
        io::stdout().flush()?;
        let summary = server.run()?;
        println!(
            "COMPLETE address={} mode=multi lobby_revision={} tables={} completed_hands={} accepted_connections={} expired_tables={} stop_reason={:?} drain_ms={} drain_checkpoint={} history_recovery={:?} safe_histories={}",
            summary.listen_addr,
            summary.lobby_revision,
            summary.tables,
            summary.completed_hands,
            summary.connections_accepted,
            summary.expired_tables,
            summary.stop_reason,
            summary.drain_millis,
            summary.drain_checkpoint_published,
            summary.history_recovery,
            summary.safe_histories
        );
        return Ok(());
    }
    let seats = TableSize::new(args.seats)?;
    let server = NetworkServer::start(NetworkServerConfig {
        bind: args.bind,
        seats,
        starting_stack: args.stack,
        deterministic_seed: args.seed,
        exit_after_hand: args.exit_after_hand,
    })?;
    let sessions = seats
        .seats()
        .map(session_id_for_seat)
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "LISTENING {} seats={} sessions={sessions}",
        server.listen_addr(),
        args.seats
    );
    io::stdout().flush()?;
    let summary = server.run()?;
    println!(
        "COMPLETE address={} revision={} stream={} accepted_connections={} disconnects={}",
        summary.listen_addr,
        summary.revision,
        summary.stream_sequence,
        summary.connections_accepted,
        summary.disconnects
    );
    Ok(())
}
