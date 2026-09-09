use std::time::Duration;

use crate::game::deck::Card;
use crate::game::hand::evaluate_best_five;
use crate::game::multiway::{MultiwayHand, MultiwayLegalActions, MultiwayPhase, PotAward};
use crate::game::seat::SeatId;
use crate::game::table::HandParticipation;
use crate::protocol::SnapshotEnvelope;

pub const SHOWDOWN_STAGE_DURATION: Duration = Duration::from_millis(1_500);
pub const SHOWDOWN_SEQUENCE_DURATION: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowdownStage {
    Reveal,
    Winners,
    Award,
}

impl ShowdownStage {
    pub fn after_reveal(elapsed: Duration) -> Self {
        if elapsed < SHOWDOWN_STAGE_DURATION {
            Self::Winners
        } else {
            Self::Award
        }
    }
    pub fn at_elapsed(elapsed: Duration) -> Option<Self> {
        match elapsed.as_millis() {
            0..=1_499 => Some(Self::Reveal),
            1_500..=2_999 => Some(Self::Winners),
            3_000..=3_999 => Some(Self::Award),
            _ => None,
        }
    }
}

pub fn terminal_hold(phase: MultiwayPhase) -> Duration {
    if phase == MultiwayPhase::HandComplete {
        Duration::from_secs(1)
    } else {
        Duration::from_millis(2_500)
    }
}

#[derive(Debug, Clone)]
pub struct ShowdownHandView {
    pub description: String,
    pub best_five: Vec<Card>,
}

#[derive(Debug, Clone)]
pub struct ProtocolReviewMetadata {
    pub version: u16,
    pub table_id: u64,
    pub hand_id: u64,
    pub revision: u64,
    pub audience: String,
    pub command_id: String,
    pub outcome: String,
}

#[derive(Debug, Clone)]
pub struct NetworkClientReviewStatus {
    pub connection: String,
    pub stream_sequence: u64,
    pub pending_command: String,
    pub deadline: String,
    pub controls: String,
}

#[derive(Debug, Clone)]
pub struct LifecycleReviewStatus {
    pub state: String,
    pub hand_active: bool,
    pub occupied: usize,
    pub eligible: usize,
    pub reservations: usize,
    pub pending: usize,
    pub boundary: String,
}

#[derive(Debug, Clone)]
pub struct MultiwayReviewSeatView {
    pub seat: SeatId,
    pub stack: u32,
    pub contribution: u32,
    pub status: String,
    pub position: String,
    pub cards: Vec<Card>,
    pub cards_visible: bool,
    pub folded: bool,
    pub showdown_hand: Option<ShowdownHandView>,
    pub winner: bool,
    pub awarded: u32,
    pub to_act: bool,
}

#[derive(Debug, Clone)]
pub struct MultiwayReviewPotView {
    pub label: String,
    pub amount: u32,
    pub eligible: Vec<SeatId>,
    pub winners: Vec<SeatId>,
}

#[derive(Debug, Clone)]
pub struct MultiwayReviewView {
    pub showdown_progress: Option<crate::game::multiway::ShowdownProgress>,
    pub publicly_shown: Vec<SeatId>,
    pub mucked: Vec<SeatId>,
    pub always_show: bool,
    pub build_id: String,
    pub hand_id: String,
    pub seed: u64,
    pub checkpoint: String,
    pub phase: MultiwayPhase,
    pub table_size: u8,
    pub board: Vec<Card>,
    pub pot_total: u32,
    pub current_wager: u32,
    pub last_full_raise_size: u32,
    pub small_blind_amount: u32,
    pub big_blind_amount: u32,
    pub ante_amount: u32,
    pub local_seat: SeatId,
    pub highlight_local_seat: bool,
    pub legal_actions: Option<MultiwayLegalActions>,
    pub seats: Vec<MultiwayReviewSeatView>,
    pub pots: Vec<MultiwayReviewPotView>,
    pub action_log: Vec<String>,
    pub protocol: Option<ProtocolReviewMetadata>,
    pub client: Option<NetworkClientReviewStatus>,
    pub lifecycle: Option<LifecycleReviewStatus>,
}

impl MultiwayReviewView {
    #[allow(clippy::too_many_arguments)]
    pub fn from_hand(
        hand: &MultiwayHand,
        build_id: &str,
        hand_id: &str,
        seed: u64,
        checkpoint: &str,
        local_seat: SeatId,
        action_log: Vec<String>,
    ) -> Self {
        let terminal = matches!(
            hand.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        );
        let mut seats = hand
            .occupied_seats()
            .map(|seat| {
                let state = hand.seat(seat);
                let mut markers = Vec::new();
                if seat == hand.button {
                    markers.push("D");
                }
                if seat == hand.small_blind {
                    markers.push("SB");
                }
                if seat == hand.big_blind {
                    markers.push("BB");
                }
                MultiwayReviewSeatView {
                    seat,
                    stack: state.stack,
                    // Chips in front of a player represent only this street.
                    // Prior-street contributions have already been swept into
                    // the public pot total.
                    contribution: state.street_contribution,
                    status: if hand.mucked_hands.contains(&seat) {
                        "MUCKED".to_string()
                    } else {
                        format!("{:?}", state.participation).to_uppercase()
                    },
                    position: markers.join("/"),
                    cards: state.hole_cards.clone(),
                    cards_visible: seat == local_seat
                        || hand
                            .revealed_hands
                            .iter()
                            .any(|revealed| revealed.seat == seat),
                    folded: state.participation == HandParticipation::Folded,
                    showdown_hand: None,
                    winner: false,
                    awarded: 0,
                    to_act: hand.to_act == Some(seat),
                }
            })
            .collect::<Vec<_>>();
        add_showdown_details(&mut seats, hand.phase, &hand.board, &hand.awards);
        let pots = hand
            .pots
            .iter()
            .enumerate()
            .map(|(index, pot)| MultiwayReviewPotView {
                label: if index == 0 {
                    "MAIN".to_string()
                } else {
                    format!("SIDE {index}")
                },
                amount: pot.amount,
                eligible: pot.eligible.clone(),
                winners: hand
                    .awards
                    .get(index)
                    .map_or_else(Vec::new, |award| award.winners.clone()),
            })
            .collect::<Vec<_>>();
        let pot_total = if terminal {
            hand.pots.iter().map(|pot| pot.amount).sum()
        } else {
            hand.occupied_seats()
                .map(|seat| hand.seat(seat).hand_contribution)
                .sum()
        };
        Self {
            showdown_progress: hand.showdown_progress.clone(),
            publicly_shown: hand.revealed_hands.iter().map(|r| r.seat).collect(),
            mucked: hand.mucked_hands.clone(),
            always_show: hand.always_show.contains(&local_seat),
            build_id: build_id.to_string(),
            hand_id: hand_id.to_string(),
            seed,
            checkpoint: checkpoint.to_string(),
            phase: hand.phase,
            table_size: hand.table_size.get(),
            board: hand.board.clone(),
            pot_total,
            current_wager: hand.current_wager,
            last_full_raise_size: hand.last_full_raise_size,
            small_blind_amount: hand.blind_values.small_blind,
            big_blind_amount: hand.blind_values.big_blind,
            ante_amount: hand.blind_values.ante,
            local_seat,
            highlight_local_seat: true,
            legal_actions: hand.legal_actions_for(local_seat),
            seats,
            pots,
            action_log,
            protocol: None,
            client: None,
            lifecycle: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_protocol_snapshot(
        hand: &MultiwayHand,
        snapshot: &SnapshotEnvelope,
        build_id: &str,
        hand_id: &str,
        seed: u64,
        checkpoint: &str,
        metadata: ProtocolReviewMetadata,
        action_log: Vec<String>,
    ) -> Self {
        let mut view = Self::from_projection(
            snapshot, build_id, hand_id, seed, checkpoint, metadata, action_log,
        );
        view.last_full_raise_size = hand.last_full_raise_size;
        view
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_projection(
        snapshot: &SnapshotEnvelope,
        build_id: &str,
        hand_id: &str,
        seed: u64,
        checkpoint: &str,
        metadata: ProtocolReviewMetadata,
        action_log: Vec<String>,
    ) -> Self {
        let projection = &snapshot.snapshot;
        let local_seat = match projection.audience {
            crate::protocol::ProjectionKind::Player { seat } => seat,
            crate::protocol::ProjectionKind::Spectator => projection.button,
        };
        let position_for = |seat: SeatId| {
            let mut markers = Vec::new();
            if seat == projection.button {
                markers.push("D");
            }
            if seat == projection.small_blind {
                markers.push("SB");
            }
            if seat == projection.big_blind {
                markers.push("BB");
            }
            markers.join("/")
        };
        let mut seats = projection
            .seats
            .iter()
            .map(|projected| MultiwayReviewSeatView {
                seat: projected.seat,
                stack: projected.stack,
                contribution: projected.street_contribution,
                status: if projection.mucked.contains(&projected.seat) {
                    "MUCKED".to_string()
                } else {
                    format!("{:?}", projected.participation).to_uppercase()
                },
                position: position_for(projected.seat),
                cards: projected.hole_cards.clone().unwrap_or_default(),
                cards_visible: projected.hole_cards.is_some(),
                folded: projected.participation == HandParticipation::Folded,
                showdown_hand: None,
                winner: false,
                awarded: 0,
                to_act: projection.to_act == Some(projected.seat),
            })
            .collect::<Vec<_>>();
        add_showdown_details(
            &mut seats,
            projection.phase,
            &projection.board,
            &projection.awards,
        );
        let pots = projection
            .pots
            .iter()
            .enumerate()
            .map(|(index, pot)| MultiwayReviewPotView {
                label: if index == 0 {
                    "MAIN".to_string()
                } else {
                    format!("SIDE {index}")
                },
                amount: pot.amount,
                eligible: pot.eligible.clone(),
                winners: projection
                    .awards
                    .get(index)
                    .map_or_else(Vec::new, |award| award.winners.clone()),
            })
            .collect();
        Self {
            showdown_progress: projection.showdown.clone(),
            publicly_shown: projection.shown.clone(),
            mucked: projection.mucked.clone(),
            always_show: projection.always_show,
            build_id: build_id.to_string(),
            hand_id: hand_id.to_string(),
            seed,
            checkpoint: checkpoint.to_string(),
            phase: projection.phase,
            table_size: projection.table_size.get(),
            board: projection.board.clone(),
            pot_total: projection.pot_total,
            current_wager: projection.current_wager,
            last_full_raise_size: 0,
            small_blind_amount: projection.small_blind_amount,
            big_blind_amount: projection.big_blind_amount,
            ante_amount: projection.ante_amount,
            local_seat,
            highlight_local_seat: matches!(
                projection.audience,
                crate::protocol::ProjectionKind::Player { .. }
            ),
            legal_actions: projection.legal_actions.clone(),
            seats,
            pots,
            action_log,
            protocol: Some(metadata),
            client: None,
            lifecycle: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_network_client(
        client: &crate::network_client::ProjectionClient,
        build_id: &str,
        hand_id: &str,
        seed: u64,
        checkpoint: &str,
        command_id: &str,
        outcome: &str,
        mut action_log: Vec<String>,
    ) -> Self {
        let snapshot = client.snapshot();
        let audience = match snapshot.snapshot.audience {
            crate::protocol::ProjectionKind::Player { seat } => {
                format!("PLAYER S{}", seat.as_u8())
            }
            crate::protocol::ProjectionKind::Spectator => "SPECTATOR".to_string(),
        };
        action_log.extend(client.activity().iter().cloned());
        let metadata = ProtocolReviewMetadata {
            version: snapshot.version,
            table_id: snapshot.table_id.0,
            hand_id: snapshot.hand_id.0,
            revision: snapshot.revision,
            audience,
            command_id: command_id.to_string(),
            outcome: outcome.to_string(),
        };
        let pending_command = client
            .pending()
            .map_or_else(|| "none".to_string(), |pending| pending.command_id.clone());
        let deadline = client.deadline().map_or_else(
            || "none / terminal".to_string(),
            |deadline| {
                format!(
                    "S{} warn {} due {}",
                    deadline.seat.as_u8(),
                    deadline.warning_tick,
                    deadline.due_tick
                )
            },
        );
        let status = NetworkClientReviewStatus {
            connection: client.connection().label().to_string(),
            stream_sequence: client.last_stream_sequence(),
            pending_command,
            deadline,
            controls: if client.controls_enabled() {
                "ENABLED"
            } else {
                "DISABLED"
            }
            .to_string(),
        };
        let mut view = Self::from_projection(
            snapshot, build_id, hand_id, seed, checkpoint, metadata, action_log,
        );
        view.client = Some(status);
        view
    }
}

fn add_showdown_details(
    seats: &mut [MultiwayReviewSeatView],
    phase: MultiwayPhase,
    board: &[Card],
    awards: &[PotAward],
) {
    let terminal = matches!(phase, MultiwayPhase::Showdown | MultiwayPhase::HandComplete);
    if !terminal {
        return;
    }
    for seat in seats {
        seat.winner = awards
            .iter()
            .any(|award| award.winners.contains(&seat.seat));
        seat.awarded = awards
            .iter()
            .flat_map(|award| &award.payouts)
            .filter(|payout| payout.seat == seat.seat)
            .map(|payout| payout.amount)
            .sum();
        if phase == MultiwayPhase::Showdown && !seat.folded && seat.cards_visible {
            seat.showdown_hand =
                evaluate_best_five(&seat.cards, board).map(|(evaluation, best_five)| {
                    ShowdownHandView {
                        description: evaluation.description,
                        best_five,
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::actions::Action;
    use crate::game::command::SeatCommand;
    use crate::game::seat::TableSize;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    #[test]
    fn displayed_chips_are_swept_from_players_at_the_street_boundary() {
        let mut hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            14,
        )
        .unwrap();
        while hand.phase == MultiwayPhase::Preflop {
            let actor = hand.to_act.unwrap();
            let legal = hand.legal_actions_for(actor).unwrap();
            let action = if legal.can_check {
                Action::Check
            } else {
                Action::Call(legal.call_amount.unwrap())
            };
            hand.apply_command(SeatCommand::new(actor, action)).unwrap();
        }
        assert_eq!(hand.phase, MultiwayPhase::Flop);
        assert!(hand
            .occupied_seats()
            .any(|player| hand.seat(player).hand_contribution > 0));
        assert!(hand
            .occupied_seats()
            .all(|player| hand.seat(player).street_contribution == 0));

        let view =
            MultiwayReviewView::from_hand(&hand, "test", "hand-1", 14, "flop", seat(0), Vec::new());
        assert!(view.seats.iter().all(|player| player.contribution == 0));
        assert!(view.pot_total > 0);
    }

    #[test]
    fn showdown_reveal_and_winners_hold_for_1500ms_before_the_one_second_award() {
        assert_eq!(
            ShowdownStage::after_reveal(Duration::from_millis(1_499)),
            ShowdownStage::Winners
        );
        assert_eq!(
            ShowdownStage::after_reveal(Duration::from_millis(1_500)),
            ShowdownStage::Award
        );
        assert_eq!(
            terminal_hold(MultiwayPhase::HandComplete),
            Duration::from_secs(1)
        );
        assert_eq!(
            terminal_hold(MultiwayPhase::Showdown),
            Duration::from_millis(2_500)
        );
        assert_eq!(
            ShowdownStage::at_elapsed(Duration::from_millis(1_499)),
            Some(ShowdownStage::Reveal)
        );
        assert_eq!(
            ShowdownStage::at_elapsed(SHOWDOWN_STAGE_DURATION),
            Some(ShowdownStage::Winners)
        );
        assert_eq!(
            ShowdownStage::at_elapsed(Duration::from_millis(2_999)),
            Some(ShowdownStage::Winners)
        );
        assert_eq!(
            ShowdownStage::at_elapsed(Duration::from_millis(3_000)),
            Some(ShowdownStage::Award)
        );
        assert_eq!(ShowdownStage::at_elapsed(SHOWDOWN_SEQUENCE_DURATION), None);
    }

    #[test]
    fn two_pair_showdown_renders_the_ace_kicker_at_standard_and_minimum_sizes() {
        use crate::game::deck::{Rank::*, Suit::*};
        use ratatui::{backend::TestBackend, Terminal};
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            14,
        )
        .unwrap();
        let mut view = MultiwayReviewView::from_hand(
            &hand,
            "regression",
            "played-hand",
            0,
            "showdown",
            seat(0),
            Vec::new(),
        );
        view.phase = MultiwayPhase::Showdown;
        view.board = [
            (Four, Spades),
            (Two, Diamonds),
            (Ace, Spades),
            (Six, Diamonds),
            (Jack, Hearts),
        ]
        .map(|(rank, suit)| Card::new(rank, suit))
        .to_vec();
        view.seats[0].cards = vec![Card::new(Jack, Spades), Card::new(Two, Hearts)];
        view.seats[0].cards_visible = true;
        let awards = [PotAward {
            pot_index: 0,
            amount: 63,
            eligible: vec![seat(0)],
            winners: vec![seat(0)],
            payouts: vec![crate::game::multiway::SeatPayout {
                seat: seat(0),
                amount: 63,
            }],
        }];
        add_showdown_details(&mut view.seats, view.phase, &view.board, &awards);
        let expected = [
            view.seats[0].cards[0],
            view.seats[0].cards[1],
            view.board[1],
            view.board[2],
            view.board[4],
        ];
        assert_eq!(
            view.seats[0].showdown_hand.as_ref().unwrap().best_five,
            expected
        );
        let expected_text = expected
            .iter()
            .map(|card| format!("[{card}]"))
            .collect::<Vec<_>>()
            .join(" ");
        for (width, height) in [(120, 40), (56, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    crate::ui::ash_table::render_with_state(
                        frame,
                        &view,
                        0,
                        None,
                        Some(ShowdownStage::Winners),
                    )
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(
                text.contains(&expected_text),
                "missing winning five at {width}x{height}"
            );
        }
    }
}
