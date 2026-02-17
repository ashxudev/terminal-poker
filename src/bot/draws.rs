use std::collections::HashSet;

use crate::game::deck::{Card, Suit};

#[derive(Debug, Clone, Default)]
pub struct DrawInfo {
    pub flush_draw: bool,
    pub oesd: bool,
    pub gutshot: bool,
    pub overcards: u8,
    pub backdoor_flush: bool,
    pub backdoor_straight: bool,
}

impl DrawInfo {
    pub fn equity_boost(&self, street_factor: f64) -> f64 {
        let mut boost = 0.0;
        if self.flush_draw {
            boost += 0.18 * street_factor;
        }
        if self.oesd {
            boost += 0.14 * street_factor;
        } else if self.gutshot {
            boost += 0.08 * street_factor;
        }
        boost += self.overcards as f64 * 0.04 * street_factor;
        if self.backdoor_flush {
            boost += 0.03 * street_factor;
        }
        if self.backdoor_straight {
            boost += 0.02 * street_factor;
        }
        boost
    }
}

pub fn detect_draws(hole_cards: &[Card], board: &[Card]) -> DrawInfo {
    if board.is_empty() {
        return DrawInfo::default();
    }

    let mut info = DrawInfo::default();

    detect_flush_draws(hole_cards, board, &mut info);
    detect_straight_draws(hole_cards, board, &mut info);
    detect_overcards(hole_cards, board, &mut info);

    info
}

fn detect_flush_draws(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let suits = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

    for &suit in &suits {
        let hole_count = hole_cards.iter().filter(|c| c.suit == suit).count();
        let board_count = board.iter().filter(|c| c.suit == suit).count();
        let total = hole_count + board_count;

        if hole_count == 0 {
            continue;
        }

        if total == 4 {
            info.flush_draw = true;
        } else if total == 3 && board.len() == 3 {
            info.backdoor_flush = true;
        }
        // total >= 5 means flush already made, not a draw
    }
}

fn detect_straight_draws(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let all_cards: Vec<&Card> = hole_cards.iter().chain(board.iter()).collect();

    // Collect unique rank values; add 1 for Ace (wheel).
    let mut rank_set: HashSet<u8> = HashSet::new();
    for card in &all_cards {
        let v = card.rank as u8;
        rank_set.insert(v);
        if v == 14 {
            rank_set.insert(1);
        }
    }

    // Hole card rank values (including Ace=1 alias).
    let mut hole_rank_values: HashSet<u8> = HashSet::new();
    for card in hole_cards {
        let v = card.rank as u8;
        hole_rank_values.insert(v);
        if v == 14 {
            hole_rank_values.insert(1);
        }
    }

    // Sliding window of 5 consecutive values.
    for base in 1..=10u8 {
        let window: Vec<u8> = (base..base + 5).collect();
        let present: Vec<u8> = window.iter().copied().filter(|v| rank_set.contains(v)).collect();
        let missing: Vec<u8> = window
            .iter()
            .copied()
            .filter(|v| !rank_set.contains(v))
            .collect();

        let hole_in_window = window.iter().any(|v| hole_rank_values.contains(v));

        if present.len() == 5 {
            // Already a straight, not a draw.
            continue;
        }

        if present.len() == 4 && missing.len() == 1 && hole_in_window {
            let gap = missing[0];
            if gap == window[0] || gap == window[4] {
                // Missing value is at either end — but only truly open-ended
                // if the *other* end can also complete a straight.
                // Window [10..=14] missing 10: J-Q-K-A, nothing above A → gutshot.
                // Window [1..=5] missing 5: A-2-3-4, nothing below A-low → gutshot.
                let is_open_ended = if gap == window[0] {
                    base + 5 <= 14 // other end (high) must be a valid rank
                } else {
                    base >= 2 // other end (low) must be a valid rank
                };
                if is_open_ended {
                    info.oesd = true;
                } else {
                    info.gutshot = true;
                }
            } else {
                // Missing value is interior: gutshot.
                info.gutshot = true;
            }
        }

        if present.len() == 3 && board.len() == 3 && hole_in_window {
            info.backdoor_straight = true;
        }
    }
}

fn detect_overcards(hole_cards: &[Card], board: &[Card], info: &mut DrawInfo) {
    let max_board_rank = board.iter().map(|c| c.rank as u8).max().unwrap_or(0);
    let count = hole_cards
        .iter()
        .filter(|c| (c.rank as u8) > max_board_rank)
        .count();
    info.overcards = count as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::Rank;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card::new(rank, suit)
    }

    #[test]
    fn test_flush_draw() {
        let hole = [card(Rank::Eight, Suit::Hearts), card(Rank::Nine, Suit::Hearts)];
        let board = [
            card(Rank::Two, Suit::Hearts),
            card(Rank::King, Suit::Spades),
            card(Rank::Five, Suit::Hearts),
        ];
        let info = detect_draws(&hole, &board);
        assert!(info.flush_draw);
        assert_eq!(info.overcards, 0);
    }

    #[test]
    fn test_no_flush_draw_when_flush_made() {
        let hole = [card(Rank::Eight, Suit::Hearts), card(Rank::Nine, Suit::Hearts)];
        let board = [
            card(Rank::Two, Suit::Hearts),
            card(Rank::King, Suit::Hearts),
            card(Rank::Five, Suit::Hearts),
        ];
        let info = detect_draws(&hole, &board);
        assert!(!info.flush_draw);
    }

    #[test]
    fn test_oesd() {
        // J-T with 9-8 on board: 8-9-T-J present, missing 7 (low end) or Q (high end)
        let hole = [card(Rank::Jack, Suit::Spades), card(Rank::Ten, Suit::Hearts)];
        let board = [
            card(Rank::Nine, Suit::Clubs),
            card(Rank::Eight, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let info = detect_draws(&hole, &board);
        assert!(info.oesd);
    }

    #[test]
    fn test_gutshot_wheel() {
        // A-5 with 3-4 on board: window [1,2,3,4,5], present=[1,3,4,5], missing=[2] (interior)
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::Five, Suit::Hearts)];
        let board = [
            card(Rank::Three, Suit::Clubs),
            card(Rank::Four, Suit::Diamonds),
            card(Rank::Eight, Suit::Spades),
        ];
        let info = detect_draws(&hole, &board);
        assert!(info.gutshot);
    }

    #[test]
    fn test_overcards() {
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::King, Suit::Hearts)];
        let board = [
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Five, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let info = detect_draws(&hole, &board);
        assert_eq!(info.overcards, 2);
    }

    #[test]
    fn test_backdoor_flush_on_flop() {
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::King, Suit::Spades)];
        let board = [
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Five, Suit::Spades),
            card(Rank::Two, Suit::Hearts),
        ];
        let info = detect_draws(&hole, &board);
        assert!(info.backdoor_flush);
    }

    #[test]
    fn test_empty_board() {
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::King, Suit::Hearts)];
        let info = detect_draws(&hole, &[]);
        assert!(!info.flush_draw);
        assert!(!info.oesd);
        assert!(!info.gutshot);
        assert_eq!(info.overcards, 0);
        assert!(!info.backdoor_flush);
        assert!(!info.backdoor_straight);
    }

    #[test]
    fn test_broadway_draw_is_gutshot_not_oesd() {
        // J-Q-K-A with board, missing T: only one end open (nothing above A)
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::King, Suit::Hearts)];
        let board = [
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Jack, Suit::Diamonds),
            card(Rank::Three, Suit::Spades),
        ];
        let info = detect_draws(&hole, &board);
        assert!(!info.oesd, "J-Q-K-A should not be OESD (one-ended)");
        assert!(info.gutshot, "J-Q-K-A should be a gutshot (needs T only)");
    }

    #[test]
    fn test_wheel_draw_is_gutshot_not_oesd() {
        // A-2-3-4 with board, missing 5: only one end open (nothing below A-low)
        let hole = [card(Rank::Ace, Suit::Spades), card(Rank::Four, Suit::Hearts)];
        let board = [
            card(Rank::Two, Suit::Clubs),
            card(Rank::Three, Suit::Diamonds),
            card(Rank::King, Suit::Spades),
        ];
        let info = detect_draws(&hole, &board);
        assert!(!info.oesd, "A-2-3-4 should not be OESD (one-ended)");
        assert!(info.gutshot, "A-2-3-4 should be a gutshot (needs 5 only)");
    }

    #[test]
    fn test_no_draws() {
        let hole = [card(Rank::Two, Suit::Spades), card(Rank::Seven, Suit::Hearts)];
        let board = [
            card(Rank::King, Suit::Clubs),
            card(Rank::Jack, Suit::Diamonds),
            card(Rank::Four, Suit::Spades),
            card(Rank::Nine, Suit::Hearts),
        ];
        let info = detect_draws(&hole, &board);
        assert!(!info.flush_draw);
        assert!(!info.oesd);
        assert!(!info.gutshot);
        assert_eq!(info.overcards, 0);
    }
}
