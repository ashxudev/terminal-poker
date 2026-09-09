use super::*;

impl MultiwayHand {
    pub(super) fn evaluate_showdown_once(&mut self) {
        if self.showdown_evaluations.is_empty() {
            self.showdown_evaluations = self
                .occupied_seats()
                .filter(|&seat| self.seat(seat).eligible_for_pot())
                .map(|seat| {
                    (
                        seat,
                        evaluate_hand(&self.seat(seat).hole_cards, &self.board),
                    )
                })
                .collect();
        }
    }
    /// Runtimes pace the same transitions that domain simulations drain.
    pub fn enable_paced_showdown(&mut self) {
        self.paced_showdown = true;
    }

    pub fn set_always_show(&mut self, seat: SeatId, value: bool) -> Result<(), CommandError> {
        if !self.occupied_seats().any(|s| s == seat) {
            return Err(CommandError::SeatNotOccupied(seat));
        }
        if !self.phase.accepts_actions() || self.showdown_progress.is_some() {
            return Err(CommandError::HandNotActive);
        }
        if value {
            self.always_show.insert(seat);
        } else {
            self.always_show.remove(&seat);
        }
        Ok(())
    }

    fn show_order(&self) -> Vec<SeatId> {
        let first = self
            .action_history
            .iter()
            .rev()
            .find(|record| {
                record.phase == MultiwayPhase::River
                    && matches!(record.action, Action::Bet(_) | Action::Raise(_))
            })
            .map(|record| record.seat);
        let mut order: Vec<_> = self
            .occupied_seats()
            .filter(|&s| self.seat(s).eligible_for_pot())
            .collect();
        order.sort_by_key(|s| clockwise_award_key(self.table_size, self.button, *s));
        if let Some(index) = first.and_then(|s| order.iter().position(|&candidate| candidate == s))
        {
            order.rotate_left(index);
        }
        order
    }

    fn table_cards(&mut self, seat: SeatId) {
        if !self.revealed_hands.iter().any(|shown| shown.seat == seat) {
            self.revealed_hands.push(RevealedHand {
                seat,
                description: self.showdown_evaluations.get(&seat).map_or_else(
                    || "All-in - board pending".to_string(),
                    |evaluation| evaluation.description.clone(),
                ),
            });
        }
    }

    /// Only already tabled hands may justify an automatic muck. Each pot in
    /// which the seat is eligible must have a strictly better tabled holding.
    fn beaten_by_tabled_hands(&self, seat: SeatId) -> bool {
        let evaluation = &self.showdown_evaluations[&seat];
        let pots = build_pots(&self.contribution_snapshot()).pots;
        let relevant: Vec<_> = pots
            .iter()
            .filter(|pot| pot.eligible.contains(&seat))
            .collect();
        !relevant.is_empty()
            && relevant.iter().all(|pot| {
                self.revealed_hands.iter().any(|shown| {
                    pot.eligible.contains(&shown.seat)
                        && compare_hands(&self.showdown_evaluations[&shown.seat], evaluation)
                            == Ordering::Greater
                })
            })
    }

    pub(super) fn reveal_synchronously(&mut self) {
        let all_in = self
            .occupied_seats()
            .any(|s| self.seat(s).participation == HandParticipation::AllIn);
        for seat in self.show_order() {
            if all_in || self.always_show.contains(&seat) || !self.beaten_by_tabled_hands(seat) {
                self.table_cards(seat);
            } else {
                self.mucked_hands.push(seat);
            }
        }
    }

    pub(super) fn begin_showdown(&mut self, all_in: bool) {
        if self.pot_eligible_count() == 1 {
            self.award_fold();
            return;
        }
        self.to_act = None;
        if !all_in {
            self.evaluate_showdown_once();
        }
        let order = self.show_order();
        self.showdown_progress = Some(ShowdownProgress {
            all_in,
            order: order.clone(),
            cursor: 0,
            mucked: Vec::new(),
        });
        if all_in {
            for seat in &order {
                self.table_cards(*seat);
            }
            self.showdown_progress.as_mut().unwrap().cursor = order.len();
        } else if let Some(&first) = order.first() {
            self.table_cards(first);
            self.showdown_progress.as_mut().unwrap().cursor = 1;
        }
        if !self.paced_showdown {
            while self.advance_showdown() {}
        }
    }

    /// One authority-only reveal, runout street, or settlement transition.
    /// No wall clock or client intention enters this domain operation.
    pub fn advance_showdown(&mut self) -> bool {
        let Some(progress) = self.showdown_progress.clone() else {
            return false;
        };
        if self.pot_eligible_count() == 1 {
            self.award_fold();
            return true;
        }
        if progress.all_in && self.board.len() < 5 {
            let count = if self.board.is_empty() { 3 } else { 1 };
            self.board.extend(self.deck.deal_n(count));
            self.phase = match self.board.len() {
                3 => MultiwayPhase::Flop,
                4 => MultiwayPhase::Turn,
                _ => MultiwayPhase::River,
            };
        } else {
            // Skip beaten holdings without adding a timer or decision for each
            // muck. Pause only when another hand actually becomes public.
            for &seat in &progress.order[progress.cursor..] {
                self.showdown_progress.as_mut().unwrap().cursor += 1;
                if self.always_show.contains(&seat) || !self.beaten_by_tabled_hands(seat) {
                    self.table_cards(seat);
                    return true;
                }
                self.mucked_hands.push(seat);
                self.showdown_progress.as_mut().unwrap().mucked = self.mucked_hands.clone();
            }
            self.showdown_progress = None;
            self.resolve_showdown();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::{Rank::*, Suit::*};

    fn s(n: u8) -> SeatId {
        SeatId::new(n).unwrap()
    }
    fn river() -> MultiwayHand {
        let mut h = MultiwayHand::new_seeded_for_review(
            TableSize::new(3).unwrap(),
            s(0),
            &[(s(0), 100), (s(1), 100), (s(2), 100)],
            1717,
        )
        .unwrap();
        while h.phase != MultiwayPhase::River {
            let actor = h.to_act.unwrap();
            let legal = h.legal_actions_for(actor).unwrap();
            h.apply_command(SeatCommand::new(
                actor,
                crate::network_client::passive_action(&legal),
            ))
            .unwrap();
        }
        h.board = vec![
            Card::new(Two, Clubs),
            Card::new(Five, Diamonds),
            Card::new(Seven, Spades),
            Card::new(Nine, Hearts),
            Card::new(Jack, Clubs),
        ];
        h.seat_mut(s(1)).hole_cards = vec![Card::new(Ace, Spades), Card::new(Ace, Hearts)];
        h.seat_mut(s(2)).hole_cards = vec![Card::new(King, Clubs), Card::new(Queen, Hearts)];
        h.seat_mut(s(0)).hole_cards = vec![Card::new(King, Diamonds), Card::new(Queen, Spades)];
        h.enable_paced_showdown();
        h
    }
    fn check_river(h: &mut MultiwayHand) {
        for actor in [s(1), s(2), s(0)] {
            h.apply_command(SeatCommand::new(actor, Action::Check))
                .unwrap();
        }
    }
    #[test]
    fn checked_river_order_and_private_losers_do_not_change_awards() {
        let mut h = river();
        check_river(&mut h);
        assert_eq!(
            h.showdown_progress.as_ref().unwrap().order,
            [s(1), s(2), s(0)]
        );
        assert_eq!(
            h.revealed_hands.iter().map(|r| r.seat).collect::<Vec<_>>(),
            [s(1)]
        );
        assert!(h.awards.is_empty());
        assert!(h.advance_showdown());
        assert_eq!(h.mucked_hands, [s(2), s(0)]);
        assert_eq!(h.awards[0].winners, [s(1)]);
        assert_eq!(h.total_chips(), 300);
        let settled = h.awards.clone();
        assert!(!h.advance_showdown());
        assert_eq!(h.awards, settled);
    }
    #[test]
    fn river_raiser_shows_first_and_winner_can_follow_a_losing_aggressor() {
        let mut h = river();
        h.apply_command(SeatCommand::new(s(1), Action::Check))
            .unwrap();
        h.apply_command(SeatCommand::new(s(2), Action::Bet(4)))
            .unwrap();
        h.apply_command(SeatCommand::new(s(0), Action::Raise(8)))
            .unwrap();
        h.apply_command(SeatCommand::new(s(1), Action::Call(8)))
            .unwrap();
        h.apply_command(SeatCommand::new(s(2), Action::Call(4)))
            .unwrap();
        assert_eq!(h.revealed_hands[0].seat, s(0));
        assert_eq!(
            h.showdown_progress.as_ref().unwrap().order,
            [s(0), s(1), s(2)]
        );
        while h.advance_showdown() {}
        assert_eq!(
            h.revealed_hands.iter().map(|r| r.seat).collect::<Vec<_>>(),
            [s(0), s(1)]
        );
        assert_eq!(h.mucked_hands, [s(2)]);
    }
    #[test]
    fn always_show_is_optional_and_cannot_change_after_reveal_starts() {
        let mut h = river();
        h.set_always_show(s(2), true).unwrap();
        check_river(&mut h);
        assert!(h.set_always_show(s(0), true).is_err());
        while h.advance_showdown() {}
        assert!(h.revealed_hands.iter().any(|r| r.seat == s(2)));
        assert_eq!(h.mucked_hands, [s(0)]);
    }
    #[test]
    fn board_ties_table_every_hand_and_preserve_chips() {
        let mut h = river();
        h.board = vec![
            Card::new(Ten, Clubs),
            Card::new(Jack, Clubs),
            Card::new(Queen, Clubs),
            Card::new(King, Clubs),
            Card::new(Ace, Clubs),
        ];
        check_river(&mut h);
        while h.advance_showdown() {}
        assert_eq!(h.revealed_hands.len(), 3);
        assert!(h.mucked_hands.is_empty());
        assert_eq!(h.awards[0].winners.len(), 3);
        assert_eq!(h.total_chips(), 300);
    }
    #[test]
    fn all_in_reveals_before_runout_across_two_to_nine_seats() {
        for count in 2..=9 {
            let size = TableSize::new(count).unwrap();
            let stacks = size.seats().map(|seat| (seat, 100)).collect::<Vec<_>>();
            let mut h = MultiwayHand::new_seeded_for_review(size, s(0), &stacks, 17).unwrap();
            h.enable_paced_showdown();
            while let Some(actor) = h.to_act {
                let target = h.legal_actions_for(actor).unwrap().all_in_to;
                h.apply_command(SeatCommand::new(actor, Action::AllIn(target)))
                    .unwrap();
            }
            assert!(h.board.is_empty());
            assert_eq!(h.revealed_hands.len(), count as usize);
            assert!(h.awards.is_empty());
            for len in [3, 4, 5] {
                assert!(h.advance_showdown());
                assert_eq!(h.board.len(), len);
                assert!(h.awards.is_empty());
            }
            assert!(h.advance_showdown());
            assert_eq!(h.phase, MultiwayPhase::Showdown);
            assert_eq!(h.total_chips(), u32::from(count) * 100);
            assert!(h.mucked_hands.is_empty());
        }
    }
    #[test]
    fn side_pot_betting_prevents_early_exposure() {
        let mut h = MultiwayHand::new_seeded_for_review(
            TableSize::new(3).unwrap(),
            s(0),
            &[(s(0), 10), (s(1), 100), (s(2), 100)],
            17,
        )
        .unwrap();
        h.enable_paced_showdown();
        for (seat, action) in [
            (s(0), Action::AllIn(10)),
            (s(1), Action::Call(9)),
            (s(2), Action::Call(8)),
        ] {
            h.apply_command(SeatCommand::new(seat, action)).unwrap();
        }
        assert_eq!(h.phase, MultiwayPhase::Flop);
        assert!(h.revealed_hands.is_empty());
        assert!(h.showdown_progress.is_none());
        h.apply_command(SeatCommand::new(s(1), Action::AllIn(90)))
            .unwrap();
        h.apply_command(SeatCommand::new(s(2), Action::AllIn(90)))
            .unwrap();
        assert_eq!(h.board.len(), 3);
        assert_eq!(h.revealed_hands.len(), 3);
        while h.advance_showdown() {}
        assert_eq!(h.awards.len(), 2);
        assert_eq!(h.total_chips(), 210);
    }
    #[test]
    fn uncalled_shoves_never_run_out_at_any_street_or_occupancy() {
        for count in 2..=9 {
            for target_phase in [
                MultiwayPhase::Preflop,
                MultiwayPhase::Flop,
                MultiwayPhase::Turn,
                MultiwayPhase::River,
            ] {
                let size = TableSize::new(count).unwrap();
                let stacks = size.seats().map(|seat| (seat, 100)).collect::<Vec<_>>();
                let mut h = MultiwayHand::new_seeded_for_review(size, s(0), &stacks, 42).unwrap();
                h.enable_paced_showdown();
                while h.phase != target_phase {
                    let actor = h.to_act.unwrap();
                    let legal = h.legal_actions_for(actor).unwrap();
                    h.apply_command(SeatCommand::new(
                        actor,
                        crate::network_client::passive_action(&legal),
                    ))
                    .unwrap();
                }
                let board = h.board.clone();
                let shover = h.to_act.unwrap();
                let target = h.legal_actions_for(shover).unwrap().all_in_to;
                h.apply_command(SeatCommand::new(shover, Action::AllIn(target)))
                    .unwrap();
                assert!(h.showdown_progress.is_none());
                while let Some(actor) = h.to_act {
                    assert_eq!(h.board, board);
                    h.apply_command(SeatCommand::new(actor, Action::Fold))
                        .unwrap();
                }
                assert_eq!(h.phase, MultiwayPhase::HandComplete);
                assert_eq!(h.board, board);
                assert!(h.revealed_hands.is_empty());
                assert!(!h.advance_showdown());
                assert_eq!(h.awards[0].winners, [shover]);
                assert_eq!(h.total_chips(), u32::from(count) * 100);
            }
        }
    }

    #[test]
    fn checked_river_starts_left_of_button_even_when_that_hand_loses() {
        let mut h = river();
        h.seat_mut(s(1)).hole_cards = vec![Card::new(Three, Hearts), Card::new(Four, Hearts)];
        h.seat_mut(s(0)).hole_cards = vec![Card::new(Ace, Hearts), Card::new(Ace, Spades)];
        check_river(&mut h);
        assert_eq!(h.revealed_hands[0].seat, s(1));
        while h.advance_showdown() {}
        assert_eq!(
            h.revealed_hands.iter().map(|r| r.seat).collect::<Vec<_>>(),
            [s(1), s(2), s(0)]
        );
        assert_eq!(h.awards[0].winners, [s(0)]);
        assert!(h.set_always_show(s(1), true).is_err());
    }

    #[test]
    fn heads_up_called_bettor_shows_and_only_a_beaten_caller_mucks() {
        for bettor_wins in [false, true] {
            for always_show in [false, true] {
                let mut h = MultiwayHand::new_seeded_for_review(
                    TableSize::new(2).unwrap(),
                    s(0),
                    &[(s(0), 100), (s(1), 100)],
                    17,
                )
                .unwrap();
                while h.phase != MultiwayPhase::River {
                    let actor = h.to_act.unwrap();
                    let legal = h.legal_actions_for(actor).unwrap();
                    h.apply_command(SeatCommand::new(
                        actor,
                        crate::network_client::passive_action(&legal),
                    ))
                    .unwrap();
                }
                h.board = river().board;
                let strong = vec![Card::new(Ace, Spades), Card::new(Ace, Hearts)];
                let weak = vec![Card::new(King, Clubs), Card::new(Queen, Hearts)];
                h.seat_mut(s(1)).hole_cards = if bettor_wins {
                    strong.clone()
                } else {
                    weak.clone()
                };
                h.seat_mut(s(0)).hole_cards = if bettor_wins { weak } else { strong };
                h.set_always_show(s(0), always_show).unwrap();
                h.enable_paced_showdown();
                h.apply_command(SeatCommand::new(s(1), Action::Bet(4)))
                    .unwrap();
                h.apply_command(SeatCommand::new(s(0), Action::Call(4)))
                    .unwrap();
                assert_eq!(h.revealed_hands[0].seat, s(1));
                let mut steps = 0;
                while h.advance_showdown() {
                    steps += 1;
                }
                assert_eq!(steps, if bettor_wins && !always_show { 1 } else { 2 });
                assert_eq!(h.awards[0].winners, [if bettor_wins { s(1) } else { s(0) }]);
                assert_eq!(
                    h.mucked_hands,
                    if bettor_wins && !always_show {
                        vec![s(0)]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(h.total_chips(), 200);
            }
        }
    }

    #[test]
    fn checked_river_order_and_automatic_completion_across_two_to_nine() {
        for count in 2..=9 {
            for button in 0..count {
                let size = TableSize::new(count).unwrap();
                let stacks = size.seats().map(|seat| (seat, 100)).collect::<Vec<_>>();
                let mut h =
                    MultiwayHand::new_seeded_for_review(size, s(button), &stacks, 42).unwrap();
                h.enable_paced_showdown();
                while let Some(actor) = h.to_act {
                    let legal = h.legal_actions_for(actor).unwrap();
                    h.apply_command(SeatCommand::new(
                        actor,
                        crate::network_client::passive_action(&legal),
                    ))
                    .unwrap();
                }
                assert_eq!(h.revealed_hands[0].seat, s((button + 1) % count));
                let mut steps = 0;
                while h.advance_showdown() {
                    steps += 1;
                    assert!(steps <= count);
                }
                for award in &h.awards {
                    for winner in &award.winners {
                        assert!(h.revealed_hands.iter().any(|shown| shown.seat == *winner));
                    }
                }
                assert_eq!(h.total_chips(), u32::from(count) * 100);
            }
        }
    }

    #[test]
    fn fold_win_has_no_showdown_or_public_cards() {
        let mut h = river();
        h.apply_command(SeatCommand::new(s(1), Action::Bet(4)))
            .unwrap();
        h.apply_command(SeatCommand::new(s(2), Action::Fold))
            .unwrap();
        h.apply_command(SeatCommand::new(s(0), Action::Fold))
            .unwrap();
        assert_eq!(h.phase, MultiwayPhase::HandComplete);
        assert!(h.showdown_progress.is_none());
        assert!(h.revealed_hands.is_empty());
        assert!(!h.advance_showdown());
    }
}
