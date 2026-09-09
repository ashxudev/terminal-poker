//! Explicit rules-path ownership for the migration period.
//!
//! [`MultiwayHand`](super::multiway::MultiwayHand) is the only supported
//! server authority. [`GameState`](super::state::GameState) is a frozen
//! two-seat offline adapter retained for the original bot-training workflow.

pub const SERVER_RULES_AUTHORITY: &str = "game::multiway::MultiwayHand";
pub const OFFLINE_COMPATIBILITY_ADAPTER: &str = "game::state::GameState (heads-up only)";

#[cfg(test)]
mod tests {
    use crate::game::actions::Action;
    use crate::game::command::SeatCommand;
    use crate::game::multiway::{MultiwayHand, MultiwayPhase};
    use crate::game::seat::{SeatId, TableSize};
    use crate::game::state::{GamePhase, GameState};

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    #[test]
    fn heads_up_adapter_and_authority_share_opening_contract() {
        let offline = GameState::new_seeded_for_review(50, 7);
        let server = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            7,
        )
        .unwrap();

        assert_eq!(offline.button, server.button);
        assert_eq!(server.small_blind, seat(0));
        assert_eq!(server.big_blind, seat(1));
        assert_eq!(offline.to_act, server.to_act.unwrap());
        assert_eq!(
            offline.street_bet(seat(0)),
            server.seat(seat(0)).street_contribution
        );
        assert_eq!(
            offline.street_bet(seat(1)),
            server.seat(seat(1)).street_contribution
        );
        assert_eq!(
            offline.pot + offline.stack(seat(0)) + offline.stack(seat(1)),
            200
        );
        assert_eq!(server.total_chips(), 200);
    }

    #[test]
    fn passive_heads_up_hand_conforms_on_street_progression_and_conservation() {
        let mut offline = GameState::new_seeded_for_review(50, 19);
        let mut server = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            19,
        )
        .unwrap();

        while !matches!(offline.phase, GamePhase::Showdown | GamePhase::HandComplete) {
            let actor = offline.to_act;
            let amount = offline.amount_to_call(actor);
            let action = if amount == 0 {
                Action::Check
            } else {
                Action::Call(amount)
            };
            offline
                .apply_command(SeatCommand::new(actor, action))
                .unwrap();
        }
        while !matches!(
            server.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            let actor = server.to_act.unwrap();
            let legal = server.legal_actions_for(actor).unwrap();
            let action = if legal.can_check {
                Action::Check
            } else if let Some(amount) = legal.call_amount {
                Action::Call(amount)
            } else {
                Action::AllIn(legal.all_in_to)
            };
            server
                .apply_command(SeatCommand::new(actor, action))
                .unwrap();
        }

        assert_eq!(offline.board.len(), 5);
        assert_eq!(server.board.len(), 5);
        assert_eq!(
            offline.stack(seat(0)) + offline.stack(seat(1)) + offline.pot,
            200
        );
        assert_eq!(server.total_chips(), 200);
    }

    #[test]
    fn path_ownership_is_machine_readable() {
        assert_eq!(
            super::SERVER_RULES_AUTHORITY,
            "game::multiway::MultiwayHand"
        );
        assert!(super::OFFLINE_COMPATIBILITY_ADAPTER.contains("heads-up only"));
    }
}
