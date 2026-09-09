//! Projection-only application state for the networked Ratatui client.

use crate::authorized_table::{SubscriptionReason, SubscriptionUpdate};
use crate::game::actions::Action;
use crate::game::multiway::MultiwayPhase;
use crate::network_client::{ProjectionClient, ProjectionClientError, UpdateDisposition};
use crate::network_transport::ServerWireMessage;
use crate::protocol::{
    CommandEnvelope, CommandOutcome, ProjectionKind, SnapshotEnvelope, TableEvent,
};
use crate::ui::multiway_review::MultiwayReviewView;

#[derive(Debug, Clone)]
pub struct NetworkApp {
    client: ProjectionClient,
    command_sequence: u64,
    command_prefix: String,
    last_command: String,
    last_outcome: String,
    transport_log: Vec<String>,
    table_console: Vec<String>,
    server_errors: u64,
}

impl NetworkApp {
    pub fn new(client: ProjectionClient, _session_label: impl Into<String>) -> Self {
        let table_console = initial_dealer_messages(client.snapshot());
        Self {
            client,
            command_sequence: 0,
            command_prefix: format!("ui-{:032x}", rand::random::<u128>()),
            last_command: "none".to_string(),
            last_outcome: "synced".to_string(),
            transport_log: vec!["WIRE  connected / authoritative welcome applied".to_string()],
            table_console,
            server_errors: 0,
        }
    }

    pub const fn client(&self) -> &ProjectionClient {
        &self.client
    }

    pub fn prepare_showdown_preference(
        &mut self,
        always_show: bool,
    ) -> Result<CommandEnvelope, ProjectionClientError> {
        self.command_sequence += 1;
        let id = format!("{}-show-{}", self.command_prefix, self.command_sequence);
        self.client.prepare_showdown_preference(id, always_show)
    }

    pub fn prepare_action(
        &mut self,
        action: Action,
    ) -> Result<CommandEnvelope, ProjectionClientError> {
        self.command_sequence += 1;
        let command_id = format!("{}-{}", self.command_prefix, self.command_sequence);
        let command = self.client.prepare_action(command_id.clone(), action)?;
        self.last_command = command_id;
        self.last_outcome = "pending".to_string();
        Ok(command)
    }

    pub fn apply_message(
        &mut self,
        message: ServerWireMessage,
    ) -> Result<(), ProjectionClientError> {
        match message {
            ServerWireMessage::LobbyWelcome { .. }
            | ServerWireMessage::Lobby { .. }
            | ServerWireMessage::LobbyError { .. } => {
                self.last_outcome = "unexpected lobby message after join".to_string();
                self.transport_log
                    .push("WIRE  rejected unexpected post-join lobby message".to_string());
            }
            ServerWireMessage::Welcome { update, .. } => {
                self.client.resynchronize_from_update(update)?;
                self.last_outcome = "reconnected / fresh snapshot".to_string();
                self.transport_log
                    .push("WIRE  reconnect welcome / resynchronized".to_string());
            }
            ServerWireMessage::Response { response } => {
                let previous_phase = self.client.snapshot().snapshot.phase;
                let console_response = response.clone();
                self.client.apply_response(response)?;
                match &console_response.receipt.outcome {
                    CommandOutcome::Accepted { event } => {
                        self.record_table_update(
                            &SubscriptionUpdate {
                                stream_sequence: console_response.stream_sequence,
                                reason: SubscriptionReason::ActionAccepted,
                                event: Some(event.clone()),
                                snapshot: console_response.snapshot,
                                deadline: console_response.deadline,
                            },
                            previous_phase,
                        );
                        self.last_outcome = "authority accepted response".to_string();
                    }
                    CommandOutcome::Rejected { error } => {
                        self.server_errors = self.server_errors.saturating_add(1);
                        self.push_table_message(format!(
                            "System · Action rejected · {}",
                            error.error.message
                        ));
                        self.last_outcome = format!("rejected {}", error.error.code.name());
                    }
                }
            }
            ServerWireMessage::Update { update } => {
                let previous_phase = self.client.snapshot().snapshot.phase;
                let console_update = update.clone();
                let disposition = self.client.apply_update(update)?;
                if disposition == UpdateDisposition::Applied {
                    self.record_table_update(&console_update, previous_phase);
                }
                self.last_outcome = "authoritative broadcast applied".to_string();
            }
            ServerWireMessage::Error { error } => {
                self.server_errors = self.server_errors.saturating_add(1);
                self.last_outcome = format!("rejected {}", error.code);
                self.transport_log.push(format!(
                    "WIRE  server error {} / {}",
                    error.code, error.message
                ));
                self.push_table_message(format!("System · Action rejected · {}", error.message));
            }
            ServerWireMessage::Goodbye => {
                self.client.mark_disconnected();
                self.last_outcome = "server closed".to_string();
                self.push_table_message("System · Connection closed · actions paused".to_string());
            }
        }
        Ok(())
    }

    pub fn mark_disconnected(&mut self) {
        self.client.mark_disconnected();
        self.last_outcome = "transport disconnected".to_string();
        self.push_table_message("System · Connection lost · actions paused".to_string());
    }

    pub const fn server_errors(&self) -> u64 {
        self.server_errors
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.client.snapshot().snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        )
    }

    pub fn view(&self, checkpoint: &str) -> MultiwayReviewView {
        let snapshot = self.client.snapshot();
        let hand_identity = format!("table-{}-hand-{}", snapshot.table_id.0, snapshot.hand_id.0);
        let mut view = MultiwayReviewView::from_network_client(
            &self.client,
            "terminal-poker-v1.0.1-sprint14-px1",
            &hand_identity,
            0,
            checkpoint,
            &self.last_command,
            &self.last_outcome,
            self.transport_log.clone(),
        );
        // Player-facing table copy is deliberately distinct from transport and
        // protocol diagnostics. Chat can later share this pane without turning
        // implementation telemetry into table conversation.
        view.action_log.clone_from(&self.table_console);
        view
    }

    fn record_table_update(&mut self, update: &SubscriptionUpdate, previous_phase: MultiwayPhase) {
        match &update.reason {
            SubscriptionReason::DeadlineWarning {
                seat,
                remaining_ticks,
            } => self.push_table_message(format!(
                "Dealer · S{} has {} ticks to act",
                seat.as_u8(),
                remaining_ticks
            )),
            SubscriptionReason::ConnectionStateChanged { seat, connected } => {
                let subject =
                    seat.map_or_else(|| "Table".to_string(), |seat| format!("S{}", seat.as_u8()));
                self.push_table_message(format!(
                    "System · {subject} {}",
                    if *connected {
                        "reconnected"
                    } else {
                        "disconnected"
                    }
                ));
            }
            SubscriptionReason::Initial
            | SubscriptionReason::ActionAccepted
            | SubscriptionReason::TimeoutAction { .. } => {}
        }

        if let Some(crate::protocol::EventEnvelope {
            event:
                TableEvent::ActionAccepted {
                    seat,
                    action,
                    pot_total,
                    ..
                },
            ..
        }) = &update.event
        {
            let local = matches!(
                update.snapshot.snapshot.audience,
                ProjectionKind::Player { seat: local } if local == *seat
            );
            let actor = if local {
                "You".to_string()
            } else {
                format!("S{}", seat.as_u8())
            };
            self.push_table_message(format!(
                "{actor} {} · Pot {pot_total}",
                player_action(*action, local)
            ));
        }

        let projection = &update.snapshot.snapshot;
        if matches!(
            update.event.as_ref().map(|event| &event.event),
            Some(TableEvent::ShowdownAdvanced)
        ) {
            if let Some(progress) = &projection.showdown {
                let message = if progress.all_in {
                    "Dealer · All-in hands tabled · Running out board".to_string()
                } else {
                    format!(
                        "Dealer · Showdown {}/{} · {} mucked",
                        progress.cursor,
                        progress.order.len(),
                        progress.mucked.len()
                    )
                };
                self.push_table_message(message);
            }
        }
        if projection.phase != previous_phase {
            let board = projection
                .board
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            self.push_table_message(format!(
                "Dealer · {}{} · Pot {}",
                projection.phase.name().to_uppercase(),
                if board.is_empty() {
                    String::new()
                } else {
                    format!(" · {board}")
                },
                projection.pot_total
            ));
        }
        if matches!(
            projection.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            for award in &projection.awards {
                for payout in &award.payouts {
                    let stack = projection
                        .seats
                        .iter()
                        .find_map(|seat| (seat.seat == payout.seat).then_some(seat.stack))
                        .unwrap_or(0);
                    self.push_table_message(format!(
                        "Dealer · S{} wins {} chips · Stack {}",
                        payout.seat.as_u8(),
                        payout.amount,
                        stack
                    ));
                }
            }
        }
    }

    fn push_table_message(&mut self, message: String) {
        self.table_console.push(message);
        if self.table_console.len() > 200 {
            self.table_console.drain(..100);
        }
    }
}

fn initial_dealer_messages(snapshot: &SnapshotEnvelope) -> Vec<String> {
    let projection = &snapshot.snapshot;
    let contribution = |target| {
        projection
            .seats
            .iter()
            .find_map(|seat| (seat.seat == target).then_some(seat.street_contribution))
            .unwrap_or(0)
    };
    vec![
        format!(
            "Dealer · Hand {} begins · Button S{}",
            snapshot.hand_id.0,
            projection.button.as_u8()
        ),
        format!(
            "Dealer · S{} posts {} · S{} posts {}",
            projection.small_blind.as_u8(),
            contribution(projection.small_blind),
            projection.big_blind.as_u8(),
            contribution(projection.big_blind)
        ),
    ]
}

fn player_action(action: Action, local: bool) -> String {
    match (action, local) {
        (Action::Fold, true) => "fold".to_string(),
        (Action::Check, true) => "check".to_string(),
        (Action::Call(amount), true) => format!("call {amount}"),
        (Action::Bet(amount), true) => format!("bet {amount}"),
        (Action::Raise(amount), true) => format!("raise to {amount}"),
        (Action::AllIn(amount), true) => format!("move all-in for {amount}"),
        (action, false) => action.description(),
    }
}
