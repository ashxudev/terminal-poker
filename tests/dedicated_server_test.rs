#[path = "support/dedicated_journey.rs"]
mod journey;
#[test]
fn lobby_access_cancellation_and_dedicated_server_complete_journey() {
    journey::run(
        std::path::Path::new(env!("CARGO_BIN_EXE_poker-server")),
        None,
    )
    .unwrap();
}
