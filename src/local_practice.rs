//! In-process, projection-driven practice authority for the installed shell.

use std::error::Error;
use std::sync::mpsc::TryRecvError;

use crate::authorized_table::{
    AuthorizedTableHandle, AuthorizedTableRuntime, AuthorizedTableSubscription, GuestSessionId,
    SessionRole,
};
use crate::game::actions::Action;
use crate::game::multiway::MultiwayHand;
use crate::game::seat::{SeatId, TableSize};
use crate::network_client::{passive_action, ProjectionClient};
use crate::network_transport::ServerWireMessage;
use crate::protocol::{HandId, ProtocolAuthority, TableId};
use crate::ring_history::SafeRingHandHistory;
use crate::ui::multiway_review::MultiwayReviewView;
use crate::ui::network_app::NetworkApp;

const LOCAL_TABLE_ID: TableId = TableId(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PracticeHandSummary {
    pub hand_number: u64,
    pub opening_stack: u32,
    pub final_stack: u32,
    pub net: i64,
    pub session_net: i64,
    pub can_continue: bool,
    pub history: SafeRingHandHistory,
}

/// A repeatable local ring-practice session.
///
/// Each hand still owns a fresh authoritative actor. Only public terminal
/// stacks, a privacy-safe history, the button, and monotonically increasing
/// hand identity cross the rollover boundary.
pub struct PracticeSession {
    table_size: TableSize,
    session_opening_stack: u32,
    current: Option<LocalPractice>,
    histories: Vec<SafeRingHandHistory>,
    table_console: Vec<String>,
    review_seed: Option<u64>,
}

impl PracticeSession {
    pub fn new(table_size: TableSize, starting_stack: u32) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            table_size,
            session_opening_stack: starting_stack,
            current: Some(LocalPractice::new(table_size, starting_stack)?),
            histories: Vec::new(),
            table_console: Vec::new(),
            review_seed: None,
        })
    }

    pub fn nine_handed(starting_stack: u32) -> Result<Self, Box<dyn Error>> {
        Self::new(TableSize::new(9)?, starting_stack)
    }

    /// Deterministic session seam for executable review evidence only.
    #[doc(hidden)]
    pub fn nine_handed_seeded_for_review(
        starting_stack: u32,
        seed: u64,
    ) -> Result<Self, Box<dyn Error>> {
        let table_size = TableSize::new(9)?;
        Ok(Self {
            table_size,
            session_opening_stack: starting_stack,
            current: Some(LocalPractice::nine_handed_seeded_for_review(
                starting_stack,
                seed,
            )?),
            histories: Vec::new(),
            table_console: Vec::new(),
            review_seed: Some(seed),
        })
    }

    pub fn current(&self) -> &LocalPractice {
        self.current
            .as_ref()
            .expect("an active practice session owns a current hand")
    }

    pub fn current_mut(&mut self) -> &mut LocalPractice {
        self.current
            .as_mut()
            .expect("an active practice session owns a current hand")
    }

    pub fn histories(&self) -> &[SafeRingHandHistory] {
        &self.histories
    }

    pub fn table_console(&self) -> &[String] {
        &self.table_console
    }

    pub fn view(&self) -> MultiwayReviewView {
        let mut view = self.current().view();
        let mut messages = self.table_console.clone();
        messages.extend(view.action_log);
        if messages.len() > 200 {
            messages.drain(..messages.len() - 200);
        }
        view.action_log = messages;
        view
    }

    pub fn complete_hand(&mut self) -> Result<PracticeHandSummary, Box<dyn Error>> {
        let completed = self
            .current
            .take()
            .ok_or("practice session has no active hand")?;
        let completed_messages = completed.view().action_log;
        let history = completed.safe_history()?;
        let opening_stack = completed.opening_stack(seat(0));
        let final_stack = history
            .final_stacks
            .iter()
            .find_map(|(player_seat, stack)| (*player_seat == seat(0)).then_some(*stack))
            .unwrap_or(0);
        let occupied = history
            .final_stacks
            .iter()
            .copied()
            .filter(|(_, stack)| *stack > 0)
            .collect::<Vec<_>>();
        let next_button = self.table_size.next_eligible(history.button, |candidate| {
            occupied
                .iter()
                .any(|(player_seat, _)| *player_seat == candidate)
        });
        let next_hand_id = HandId(history.hand_id.0.saturating_add(1));
        let can_continue = final_stack > 0 && occupied.len() >= 2 && next_button.is_some();
        let net = i64::from(final_stack) - i64::from(opening_stack);
        let session_net = i64::from(final_stack) - i64::from(self.session_opening_stack);
        self.table_console.extend(completed_messages);
        self.table_console.push(format!(
            "Dealer · Hand {} complete · You {} · Stack {} · Session {:+}",
            history.hand_id.0,
            if net > 0 {
                format!("win {net}")
            } else if net < 0 {
                format!("lose {}", -net)
            } else {
                "break even".to_string()
            },
            final_stack,
            session_net
        ));
        if can_continue {
            self.table_console
                .push("Dealer · Next hand starting automatically".to_string());
        } else {
            self.table_console
                .push("Dealer · Practice session complete".to_string());
        }
        if self.table_console.len() > 200 {
            self.table_console.drain(..self.table_console.len() - 200);
        }
        self.histories.push(history.clone());

        // Stop the completed actor before reusing the local table identity.
        drop(completed);
        if can_continue {
            self.current = Some(LocalPractice::from_stacks(
                self.table_size,
                next_button.expect("continuation requires an eligible next button"),
                next_hand_id,
                &occupied,
                self.review_seed
                    .map(|seed| seed.saturating_add(next_hand_id.0)),
            )?);
        }

        Ok(PracticeHandSummary {
            hand_number: history.hand_id.0,
            opening_stack,
            final_stack,
            net,
            session_net,
            can_continue,
            history,
        })
    }
}

pub struct LocalPractice {
    _runtime: AuthorizedTableRuntime,
    handle: AuthorizedTableHandle,
    local_session: GuestSessionId,
    subscription: AuthorizedTableSubscription,
    app: NetworkApp,
    table_size: TableSize,
    hand_id: HandId,
    button: SeatId,
    opening_stacks: Vec<(SeatId, u32)>,
    bot_command_sequence: u64,
}

impl LocalPractice {
    pub fn new(table_size: TableSize, starting_stack: u32) -> Result<Self, Box<dyn Error>> {
        let stacks = table_size
            .seats()
            .map(|seat| (seat, starting_stack))
            .collect::<Vec<_>>();
        Self::from_stacks(table_size, seat(0), HandId(1), &stacks, None)
    }

    #[doc(hidden)]
    pub fn nine_handed_seeded_for_review(
        starting_stack: u32,
        seed: u64,
    ) -> Result<Self, Box<dyn Error>> {
        let table_size = TableSize::new(9)?;
        let stacks = table_size
            .seats()
            .map(|player_seat| (player_seat, starting_stack))
            .collect::<Vec<_>>();
        Self::from_stacks(table_size, seat(0), HandId(1), &stacks, Some(seed))
    }

    fn from_stacks(
        table_size: TableSize,
        button: SeatId,
        hand_id: HandId,
        stacks: &[(SeatId, u32)],
        review_seed: Option<u64>,
    ) -> Result<Self, Box<dyn Error>> {
        let hand = match review_seed {
            Some(seed) => MultiwayHand::new_seeded_for_review(table_size, button, stacks, seed)?,
            None => MultiwayHand::new(table_size, button, stacks)?,
        };
        let runtime = AuthorizedTableRuntime::spawn(if review_seed.is_some() {
            ProtocolAuthority::new(LOCAL_TABLE_ID, hand_id, hand)
        } else {
            ProtocolAuthority::new_paced(LOCAL_TABLE_ID, hand_id, hand)
        })?;
        let handle = runtime.handle();
        for &(player_seat, _) in stacks {
            handle.bind(
                session_for(player_seat)?,
                LOCAL_TABLE_ID,
                hand_id,
                SessionRole::Player { seat: player_seat },
            )?;
        }

        let local_session = session_for(seat(0))?;
        let subscription = handle.subscribe(local_session.clone())?;
        let initial = subscription.recv()?;
        let client = ProjectionClient::bootstrap_from_update(initial)?;
        let app = NetworkApp::new(client, "practice-player");
        Ok(Self {
            _runtime: runtime,
            handle,
            local_session,
            subscription,
            app,
            table_size,
            hand_id,
            button,
            opening_stacks: stacks.to_vec(),
            bot_command_sequence: 0,
        })
    }

    pub fn nine_handed(starting_stack: u32) -> Result<Self, Box<dyn Error>> {
        Self::new(TableSize::new(9)?, starting_stack)
    }

    pub const fn app(&self) -> &NetworkApp {
        &self.app
    }

    pub const fn hand_id(&self) -> HandId {
        self.hand_id
    }

    pub const fn button(&self) -> SeatId {
        self.button
    }

    pub fn opening_stack(&self, player_seat: SeatId) -> u32 {
        self.opening_stacks
            .iter()
            .find_map(|(candidate, stack)| (*candidate == player_seat).then_some(*stack))
            .unwrap_or(0)
    }

    pub fn view(&self) -> MultiwayReviewView {
        self.app.view("QUICK PRACTICE")
    }

    pub fn apply_updates(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            match self.subscription.try_recv() {
                Ok(update) => self
                    .app
                    .apply_message(ServerWireMessage::Update { update })?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => {
                    self.app.mark_disconnected();
                    return Ok(());
                }
            }
        }
    }

    pub fn submit_local(&mut self, action: Action) -> Result<(), Box<dyn Error>> {
        let command = self.app.prepare_action(action)?;
        let response = self.handle.submit(self.local_session.clone(), command)?;
        self.app
            .apply_message(ServerWireMessage::Response { response })?;
        self.apply_updates()
    }

    pub fn set_showdown_preference(&mut self, always_show: bool) -> Result<(), Box<dyn Error>> {
        let command = self.app.prepare_showdown_preference(always_show)?;
        let response = self.handle.submit(self.local_session.clone(), command)?;
        self.app
            .apply_message(ServerWireMessage::Response { response })?;
        self.apply_updates()
    }

    /// Applies at most one bot action through the same authorized command path.
    pub fn step_bot(&mut self) -> Result<bool, Box<dyn Error>> {
        self.apply_updates()?;
        if self.app.is_terminal() {
            return Ok(false);
        }
        let Some(actor) = self.app.client().snapshot().snapshot.to_act else {
            return Ok(false);
        };
        if actor == seat(0) {
            return Ok(false);
        }
        if !self.table_size.contains(actor) {
            return Err("authority selected an actor outside the practice table".into());
        }

        let bot_session = session_for(actor)?;
        let snapshot = self.handle.snapshot(bot_session.clone())?;
        let stream_sequence = self.handle.metrics()?.stream_sequence;
        let mut client = ProjectionClient::bootstrap(snapshot, stream_sequence)?;
        let legal = client
            .snapshot()
            .snapshot
            .legal_actions
            .as_ref()
            .ok_or("bot actor projection contains no legal actions")?;
        let action = passive_action(legal);
        self.bot_command_sequence = self.bot_command_sequence.saturating_add(1);
        let command = client.prepare_action(
            format!(
                "practice-bot-{}-{}",
                actor.as_u8(),
                self.bot_command_sequence
            ),
            action,
        )?;
        self.handle.submit(bot_session, command)?;
        self.apply_updates()?;
        Ok(true)
    }

    pub fn safe_history(&self) -> Result<SafeRingHandHistory, Box<dyn Error>> {
        if !self.app.is_terminal() {
            return Err("practice hand must be terminal before history capture".into());
        }
        let (terminal, accepted_events) = self.handle.safe_history_material()?;
        Ok(SafeRingHandHistory::from_public_terminal(
            &terminal,
            &accepted_events,
        )?)
    }
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("local practice seat is within the supported range")
}

fn session_for(player_seat: SeatId) -> Result<GuestSessionId, Box<dyn Error>> {
    Ok(GuestSessionId::new(format!(
        "local-practice-seat-{}",
        player_seat.as_u8()
    ))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_handed_practice_starts_from_one_private_projection() {
        let practice = LocalPractice::nine_handed(100).unwrap();
        let view = practice.view();

        assert_eq!(view.table_size, 9);
        assert_eq!(view.seats.len(), 9);
        assert_eq!(
            view.seats.iter().filter(|seat| seat.cards_visible).count(),
            1
        );
        assert_eq!(
            view.seats.iter().filter(|seat| !seat.cards_visible).count(),
            8
        );
        assert!(view
            .seats
            .iter()
            .filter(|player| player.seat != seat(0))
            .all(|player| player.cards.is_empty()));
    }

    #[test]
    fn table_console_uses_player_facing_dealer_copy_not_protocol_telemetry() {
        let mut practice = LocalPractice::nine_handed(100).unwrap();
        let initial = practice.view().action_log.join("\n");
        assert!(initial.contains("Dealer · Hand 1 begins · Button S0"));
        assert!(initial.contains("Dealer · S1 posts 1 · S2 posts 2"));
        for diagnostic in ["WIRE", "STREAM", "SYNC", "INTENT", "revision"] {
            assert!(!initial.contains(diagnostic));
        }

        assert!(practice.step_bot().unwrap());
        let after_action = practice.view().action_log.join("\n");
        assert!(after_action.contains("S3 calls 2 · Pot 5"));
    }

    #[test]
    fn local_responses_print_the_player_action_street_and_terminal_award() {
        let mut practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001).unwrap();
        for _ in 0..200 {
            practice.apply_updates().unwrap();
            if practice.view().showdown_progress.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            if practice.app().is_terminal() {
                break;
            }
            if practice.app().client().controls_enabled() {
                let legal = practice
                    .app()
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap();
                practice.submit_local(passive_action(legal)).unwrap();
            } else {
                practice.step_bot().unwrap();
            }
        }

        assert!(practice.app().is_terminal());
        let console = practice.view().action_log;
        assert!(console.iter().any(|line| line == "You call 2 · Pot 17"));
        assert!(console.iter().any(|line| line.starts_with("Dealer · FLOP")));
        assert!(console
            .iter()
            .any(|line| line.starts_with("Dealer · S") && line.contains("wins 18 chips")));
    }

    #[test]
    fn local_view_cannot_observe_a_bot_private_hand_or_authority_secrets() {
        let practice = LocalPractice::nine_handed(100).unwrap();
        let bot_snapshot = practice
            .handle
            .snapshot(session_for(seat(1)).unwrap())
            .unwrap();
        let bot_cards = bot_snapshot.snapshot.seats[1]
            .hole_cards
            .clone()
            .expect("S1 sees its own private cards");
        let local_view = practice.view();
        let local_s1 = local_view
            .seats
            .iter()
            .find(|item| item.seat == seat(1))
            .unwrap();

        assert!(!local_s1.cards_visible);
        assert!(local_s1.cards.is_empty());
        assert_eq!(bot_cards.len(), 2);
        let debug = format!("{local_view:?}");
        for card in bot_cards {
            assert!(!debug.contains(&format!("{card:?}")));
        }
        for forbidden in [
            "deck_order",
            "random_state",
            "reconnect_token",
            "deal_plan",
            "policy_state",
        ] {
            assert!(!debug.contains(forbidden));
        }
    }

    #[test]
    fn every_practice_action_crosses_the_authoritative_command_boundary() {
        let mut practice = LocalPractice::nine_handed(100).unwrap();
        let before = practice.handle.metrics().unwrap().actor.accepted_commands;
        let acted = if practice.app().client().controls_enabled() {
            let legal = practice
                .app()
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .unwrap();
            practice.submit_local(passive_action(legal)).unwrap();
            true
        } else {
            practice.step_bot().unwrap()
        };
        assert!(acted);
        assert_eq!(
            practice.handle.metrics().unwrap().actor.accepted_commands,
            before + 1
        );
    }

    #[test]
    fn passive_nine_handed_practice_terminates_and_conserves_chips() {
        let mut practice = LocalPractice::nine_handed(100).unwrap();
        for _ in 0..200 {
            practice.apply_updates().unwrap();
            if practice.view().showdown_progress.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            if practice.app().is_terminal() {
                break;
            }
            if practice.app().client().controls_enabled() {
                let legal = practice
                    .app()
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap();
                practice.submit_local(passive_action(legal)).unwrap();
            } else {
                practice.step_bot().unwrap();
            }
        }

        assert!(practice.app().is_terminal());
        assert_eq!(
            practice
                .view()
                .seats
                .iter()
                .map(|seat| seat.stack)
                .sum::<u32>(),
            900
        );
    }

    fn finish_passively(session: &mut PracticeSession) -> PracticeHandSummary {
        for _ in 0..200 {
            let practice = session.current_mut();
            practice.apply_updates().unwrap();
            if practice.view().showdown_progress.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            if practice.app().is_terminal() {
                break;
            }
            if practice.app().client().controls_enabled() {
                let legal = practice
                    .app()
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap();
                practice.submit_local(passive_action(legal)).unwrap();
            } else {
                practice.step_bot().unwrap();
            }
        }
        assert!(session.current().app().is_terminal());
        session.complete_hand().unwrap()
    }

    #[test]
    fn three_hand_session_preserves_stacks_advances_button_and_records_safe_history() {
        let mut session = PracticeSession::nine_handed(100).unwrap();
        let mut summaries = Vec::new();
        for _ in 0..3 {
            summaries.push(finish_passively(&mut session));
        }

        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.hand_number)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.history.button.as_u8())
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(summaries.iter().all(|summary| summary.can_continue));
        assert_eq!(session.histories().len(), 3);
        assert_eq!(session.current().hand_id(), HandId(4));
        assert_eq!(session.current().button(), seat(3));
        assert_eq!(
            session
                .current()
                .view()
                .seats
                .iter()
                .map(|player| player.stack)
                .sum::<u32>()
                + session.current().view().pot_total,
            900
        );
        assert_eq!(
            session
                .histories()
                .iter()
                .map(|history| history
                    .final_stacks
                    .iter()
                    .map(|(_, stack)| stack)
                    .sum::<u32>())
                .collect::<Vec<_>>(),
            vec![900, 900, 900]
        );
    }

    #[test]
    fn completed_hand_summary_stays_in_the_table_console_as_the_next_hand_starts() {
        let mut session = PracticeSession::nine_handed(100).unwrap();
        let summary = finish_passively(&mut session);
        assert!(summary.can_continue);

        let view = session.view();
        let console = view.action_log.join("\n");
        assert!(console.contains("Dealer · Hand 1 complete · You "));
        assert!(console.contains(&format!("· Stack {} ·", summary.final_stack)));
        assert!(console.contains(&format!("· Session {:+}", summary.session_net)));
        assert!(console.contains("Dealer · Next hand starting automatically"));
        assert!(console.contains("Dealer · Hand 2 begins · Button S1"));
        assert!(!console.contains("RESULTS"));
        assert_eq!(session.current().hand_id(), HandId(2));
    }

    #[test]
    fn ten_practice_authorities_drop_cleanly_after_partial_gameplay() {
        for _ in 0..10 {
            let mut practice = LocalPractice::nine_handed(100).unwrap();
            let progressed = if practice.app().client().controls_enabled() {
                let legal = practice
                    .app()
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap();
                practice.submit_local(passive_action(legal)).unwrap();
                true
            } else {
                practice.step_bot().unwrap()
            };
            assert!(progressed);
            assert_eq!(
                practice.handle.metrics().unwrap().actor.accepted_commands,
                1
            );
        }
    }
}
