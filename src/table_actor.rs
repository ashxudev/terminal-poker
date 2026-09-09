//! One bounded, serialized mutation authority for an active poker table.
//!
//! The actor is intentionally transport-neutral. A single worker owns the
//! complete `ProtocolAuthority`; cloned handles can only enqueue bounded
//! requests and receive public outcomes and audience-specific projections.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

use crate::protocol::{
    decode_command_json, AcknowledgementDelivery, AcknowledgementResult, CommandEnvelope,
    DecodeCommandError, ProjectionAudience, ProjectionError, ProtocolAuthority, SnapshotEnvelope,
    SubmissionReceipt,
};

pub const TABLE_MAILBOX_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableActorMetrics {
    pub processed_commands: u64,
    pub accepted_commands: u64,
    pub rejected_commands: u64,
    pub replayed_commands: u64,
    pub decode_rejections: u64,
    pub snapshot_requests: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableActorResponse {
    pub receipt: SubmissionReceipt,
    pub snapshot: SnapshotEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableActorError {
    MailboxClosed,
    ResponseChannelClosed,
    Projection(ProjectionError),
    Decode(DecodeCommandError),
    WorkerPanicked,
}

impl Display for TableActorError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MailboxClosed => formatter.write_str("table actor mailbox is closed"),
            Self::ResponseChannelClosed => {
                formatter.write_str("table actor response channel is closed")
            }
            Self::Projection(error) => write!(formatter, "projection failed: {error:?}"),
            Self::Decode(error) => write!(formatter, "{}: {}", error.code.name(), error.message),
            Self::WorkerPanicked => formatter.write_str("table actor worker panicked"),
        }
    }
}

impl Error for TableActorError {}

#[derive(Debug, Clone)]
pub struct TableActorHandle {
    sender: SyncSender<TableActorRequest>,
}

impl TableActorHandle {
    pub(crate) fn advance_showdown(
        &self,
    ) -> Result<Option<crate::protocol::EventEnvelope>, TableActorError> {
        let (respond_to, receiver) = mpsc::channel();
        self.sender
            .send(TableActorRequest::AdvanceShowdown { respond_to })
            .map_err(|_| TableActorError::MailboxClosed)?;
        receiver
            .recv()
            .map_err(|_| TableActorError::ResponseChannelClosed)
    }
    pub fn submit(
        &self,
        command: CommandEnvelope,
        audience: ProjectionAudience,
    ) -> Result<TableActorResponse, TableActorError> {
        let (response_sender, response_receiver) = mpsc::channel();
        self.sender
            .send(TableActorRequest::Submit {
                command,
                audience,
                server_generated: false,
                respond_to: response_sender,
            })
            .map_err(|_| TableActorError::MailboxClosed)?;
        response_receiver
            .recv()
            .map_err(|_| TableActorError::ResponseChannelClosed)?
    }

    pub(crate) fn submit_server(
        &self,
        command: CommandEnvelope,
        audience: ProjectionAudience,
    ) -> Result<TableActorResponse, TableActorError> {
        let (response_sender, response_receiver) = mpsc::channel();
        self.sender
            .send(TableActorRequest::Submit {
                command,
                audience,
                server_generated: true,
                respond_to: response_sender,
            })
            .map_err(|_| TableActorError::MailboxClosed)?;
        response_receiver
            .recv()
            .map_err(|_| TableActorError::ResponseChannelClosed)?
    }

    pub fn submit_json(
        &self,
        bytes: &[u8],
        audience: ProjectionAudience,
    ) -> Result<TableActorResponse, TableActorError> {
        match decode_command_json(bytes) {
            Ok(command) => self.submit(command, audience),
            Err(error) => {
                let (response_sender, response_receiver) = mpsc::channel();
                self.sender
                    .send(TableActorRequest::DecodeRejected {
                        error,
                        respond_to: response_sender,
                    })
                    .map_err(|_| TableActorError::MailboxClosed)?;
                Err(response_receiver
                    .recv()
                    .map_err(|_| TableActorError::ResponseChannelClosed)?)
            }
        }
    }

    pub fn snapshot(
        &self,
        audience: ProjectionAudience,
    ) -> Result<SnapshotEnvelope, TableActorError> {
        let (response_sender, response_receiver) = mpsc::channel();
        self.sender
            .send(TableActorRequest::Snapshot {
                audience,
                respond_to: response_sender,
            })
            .map_err(|_| TableActorError::MailboxClosed)?;
        response_receiver
            .recv()
            .map_err(|_| TableActorError::ResponseChannelClosed)?
            .map_err(TableActorError::Projection)
    }

    pub fn metrics(&self) -> Result<TableActorMetrics, TableActorError> {
        let (response_sender, response_receiver) = mpsc::channel();
        self.sender
            .send(TableActorRequest::Metrics {
                respond_to: response_sender,
            })
            .map_err(|_| TableActorError::MailboxClosed)?;
        response_receiver
            .recv()
            .map_err(|_| TableActorError::ResponseChannelClosed)
    }
}

#[derive(Debug)]
pub struct TableActor {
    handle: TableActorHandle,
    worker: Option<JoinHandle<()>>,
}

impl TableActor {
    pub fn spawn(authority: ProtocolAuthority) -> Result<Self, TableActorError> {
        let (sender, receiver) = mpsc::sync_channel(TABLE_MAILBOX_CAPACITY);
        let worker = thread::Builder::new()
            .name(format!("table-actor-{}", authority.table_id().0))
            .spawn(move || run_actor(receiver, authority))
            .map_err(|_| TableActorError::MailboxClosed)?;
        Ok(Self {
            handle: TableActorHandle { sender },
            worker: Some(worker),
        })
    }

    pub fn handle(&self) -> TableActorHandle {
        self.handle.clone()
    }

    pub fn shutdown(mut self) -> Result<(), TableActorError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), TableActorError> {
        if let Some(worker) = self.worker.take() {
            let _ = self.handle.sender.send(TableActorRequest::Shutdown);
            worker.join().map_err(|_| TableActorError::WorkerPanicked)?;
        }
        Ok(())
    }
}

impl Drop for TableActor {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

enum TableActorRequest {
    AdvanceShowdown {
        respond_to: mpsc::Sender<Option<crate::protocol::EventEnvelope>>,
    },
    Submit {
        command: CommandEnvelope,
        audience: ProjectionAudience,
        server_generated: bool,
        respond_to: mpsc::Sender<Result<TableActorResponse, TableActorError>>,
    },
    DecodeRejected {
        error: DecodeCommandError,
        respond_to: mpsc::Sender<TableActorError>,
    },
    Snapshot {
        audience: ProjectionAudience,
        respond_to: mpsc::Sender<Result<SnapshotEnvelope, ProjectionError>>,
    },
    Metrics {
        respond_to: mpsc::Sender<TableActorMetrics>,
    },
    Shutdown,
}

fn run_actor(receiver: Receiver<TableActorRequest>, mut authority: ProtocolAuthority) {
    let mut metrics = TableActorMetrics::default();
    while let Ok(request) = receiver.recv() {
        match request {
            TableActorRequest::AdvanceShowdown { respond_to } => {
                let _ = respond_to.send(authority.advance_showdown());
            }
            TableActorRequest::Submit {
                command,
                audience,
                server_generated,
                respond_to,
            } => {
                if let Err(error) = authority.snapshot(audience) {
                    let _ = respond_to.send(Err(TableActorError::Projection(error)));
                    continue;
                }
                let receipt = if server_generated {
                    authority.submit_server_with_acknowledgement(command)
                } else {
                    authority.submit_with_acknowledgement(command)
                };
                metrics.processed_commands += 1;
                match (
                    receipt.acknowledgement.delivery,
                    receipt.acknowledgement.result,
                ) {
                    (AcknowledgementDelivery::Replayed, _) => metrics.replayed_commands += 1,
                    (AcknowledgementDelivery::Processed, AcknowledgementResult::Accepted) => {
                        metrics.accepted_commands += 1
                    }
                    (AcknowledgementDelivery::Processed, AcknowledgementResult::Rejected) => {
                        metrics.rejected_commands += 1
                    }
                }
                let response = authority
                    .snapshot(audience)
                    .map(|snapshot| TableActorResponse { receipt, snapshot })
                    .map_err(TableActorError::Projection);
                let _ = respond_to.send(response);
            }
            TableActorRequest::DecodeRejected { error, respond_to } => {
                metrics.decode_rejections += 1;
                let _ = respond_to.send(TableActorError::Decode(error));
            }
            TableActorRequest::Snapshot {
                audience,
                respond_to,
            } => {
                metrics.snapshot_requests += 1;
                let _ = respond_to.send(authority.snapshot(audience));
            }
            TableActorRequest::Metrics { respond_to } => {
                let _ = respond_to.send(metrics);
            }
            TableActorRequest::Shutdown => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::*;
    use crate::game::actions::Action;
    use crate::game::multiway::MultiwayHand;
    use crate::game::seat::{SeatId, TableSize};
    use crate::protocol::{
        AcknowledgementDelivery, CommandOutcome, HandId, ProjectionKind, ProtocolErrorCode,
        TableId, MAX_COMMAND_ENVELOPE_BYTES,
    };

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn actor() -> TableActor {
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(4).unwrap(),
            seat(0),
            &[
                (seat(0), 40),
                (seat(1), 100),
                (seat(2), 200),
                (seat(3), 200),
            ],
            13,
        )
        .unwrap();
        TableActor::spawn(ProtocolAuthority::new(TableId(44), HandId(1), hand)).unwrap()
    }

    #[test]
    fn actor_returns_audience_specific_snapshot_and_public_safe_metrics() {
        let actor = actor();
        let handle = actor.handle();
        let response = handle
            .submit(
                CommandEnvelope::act("actor-1", TableId(44), 0, seat(3), Action::AllIn(200)),
                ProjectionAudience::Player(seat(0)),
            )
            .unwrap();
        assert_eq!(response.snapshot.revision, 1);
        assert_eq!(
            response.snapshot.snapshot.audience,
            ProjectionKind::Player { seat: seat(0) }
        );
        assert!(response.snapshot.snapshot.seats[0].hole_cards.is_some());
        assert!(response.snapshot.snapshot.seats[1..]
            .iter()
            .all(|seat| seat.hole_cards.is_none()));
        assert_eq!(
            handle.metrics().unwrap(),
            TableActorMetrics {
                processed_commands: 1,
                accepted_commands: 1,
                ..TableActorMetrics::default()
            }
        );
        actor.shutdown().unwrap();
    }

    #[test]
    fn concurrent_exact_retries_apply_one_transition_and_replay_the_rest() {
        const SENDERS: usize = 12;
        let actor = actor();
        let handle = actor.handle();
        let barrier = Arc::new(Barrier::new(SENDERS));
        let command = CommandEnvelope::act(
            "concurrent-retry",
            TableId(44),
            0,
            seat(3),
            Action::AllIn(200),
        );
        let workers = (0..SENDERS)
            .map(|_| {
                let worker_handle = handle.clone();
                let worker_barrier = Arc::clone(&barrier);
                let worker_command = command.clone();
                thread::spawn(move || {
                    worker_barrier.wait();
                    worker_handle
                        .submit(worker_command, ProjectionAudience::Player(seat(0)))
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let responses = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            responses
                .iter()
                .filter(|response| {
                    response.receipt.acknowledgement.delivery == AcknowledgementDelivery::Processed
                })
                .count(),
            1
        );
        assert!(responses
            .iter()
            .all(|response| response.snapshot.revision == 1));
        let metrics = handle.metrics().unwrap();
        assert_eq!(metrics.processed_commands, SENDERS as u64);
        assert_eq!(metrics.accepted_commands, 1);
        assert_eq!(metrics.replayed_commands, (SENDERS - 1) as u64);
        assert_eq!(metrics.rejected_commands, 0);
        actor.shutdown().unwrap();
    }

    #[test]
    fn concurrent_distinct_commands_cannot_both_mutate_one_revision() {
        let actor = actor();
        let barrier = Arc::new(Barrier::new(2));
        let commands = [
            CommandEnvelope::act("race-all-in", TableId(44), 0, seat(3), Action::AllIn(200)),
            CommandEnvelope::act("race-raise", TableId(44), 0, seat(3), Action::Raise(10)),
        ];
        let workers = commands.map(|command| {
            let worker_handle = actor.handle();
            let worker_barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                worker_barrier.wait();
                worker_handle
                    .submit(command, ProjectionAudience::Spectator)
                    .unwrap()
            })
        });
        let responses = workers.map(|worker| worker.join().unwrap());
        let accepted = responses
            .iter()
            .filter(|response| matches!(response.receipt.outcome, CommandOutcome::Accepted { .. }))
            .count();
        let stale = responses
            .iter()
            .filter(|response| match &response.receipt.outcome {
                CommandOutcome::Rejected { error } => {
                    error.error.code == ProtocolErrorCode::StaleRevision
                }
                CommandOutcome::Accepted { .. } => false,
            })
            .count();
        assert_eq!((accepted, stale), (1, 1));
        assert!(responses
            .iter()
            .all(|response| response.snapshot.revision == 1));
        let metrics = actor.handle().metrics().unwrap();
        assert_eq!(metrics.accepted_commands, 1);
        assert_eq!(metrics.rejected_commands, 1);
        actor.shutdown().unwrap();
    }

    #[test]
    fn malformed_and_oversized_json_are_counted_without_reaching_authority() {
        let actor = actor();
        let handle = actor.handle();
        let malformed = handle
            .submit_json(b"{", ProjectionAudience::Spectator)
            .unwrap_err();
        let oversized = handle
            .submit_json(
                &vec![b'x'; MAX_COMMAND_ENVELOPE_BYTES + 1],
                ProjectionAudience::Spectator,
            )
            .unwrap_err();
        assert!(matches!(malformed, TableActorError::Decode(_)));
        assert!(matches!(oversized, TableActorError::Decode(_)));
        let snapshot = handle.snapshot(ProjectionAudience::Spectator).unwrap();
        assert_eq!(snapshot.revision, 0);
        let metrics = handle.metrics().unwrap();
        assert_eq!(metrics.decode_rejections, 2);
        assert_eq!(metrics.processed_commands, 0);
        actor.shutdown().unwrap();
    }
}
