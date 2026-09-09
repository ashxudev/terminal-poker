use super::deck::{Card, Rank};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard = 0,
    Pair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandEvaluation {
    pub rank: HandRank,
    /// Category-defining ranks followed by every remaining kicker, strongest first.
    pub kickers: Vec<Rank>,
    pub description: String,
}

impl HandEvaluation {
    /// Returns a normalized strength value between 0.0 and 1.0
    pub fn strength(&self) -> f64 {
        let base = self.rank as u32 as f64 / 8.0;
        let kicker_bonus = if !self.kickers.is_empty() {
            (self.kickers[0] as u32 as f64 - 2.0) / 12.0 * 0.1
        } else {
            0.0
        };
        (base + kicker_bonus).min(1.0)
    }
}

pub fn evaluate_hand(hole_cards: &[Card], board: &[Card]) -> HandEvaluation {
    let mut all_cards: Vec<Card> = hole_cards.to_vec();
    all_cards.extend(board);

    if all_cards.len() < 5 {
        return evaluate_partial(&all_cards);
    }

    evaluate_best_five(hole_cards, board)
        .map(|(evaluation, _)| evaluation)
        .unwrap_or_else(|| HandEvaluation {
            rank: HandRank::HighCard,
            kickers: vec![],
            description: "Unknown".to_string(),
        })
}

/// Returns the evaluated hand and one deterministic five-card realization of
/// it. This is suitable for showing which cards play after those cards are
/// legitimately visible; callers must still enforce projection privacy.
pub fn evaluate_best_five(
    hole_cards: &[Card],
    board: &[Card],
) -> Option<(HandEvaluation, Vec<Card>)> {
    let mut all_cards = hole_cards.to_vec();
    all_cards.extend(board);
    let mut best: Option<(HandEvaluation, Vec<Card>)> = None;
    for cards in combinations(&all_cards, 5) {
        let evaluation = evaluate_five(&cards);
        let replaces = best.as_ref().is_none_or(|(current, _)| {
            evaluation
                .rank
                .cmp(&current.rank)
                .then_with(|| evaluation.kickers.cmp(&current.kickers))
                .is_gt()
        });
        if replaces {
            best = Some((evaluation, cards));
        }
    }
    best
}

fn evaluate_partial(cards: &[Card]) -> HandEvaluation {
    // Handle empty cards case
    if cards.is_empty() {
        return HandEvaluation {
            rank: HandRank::HighCard,
            kickers: vec![],
            description: "No cards".to_string(),
        };
    }

    // For partial boards, estimate hand strength
    let mut rank_counts: HashMap<Rank, u8> = HashMap::new();
    for card in cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
    }

    let mut pairs = 0;
    let mut trips = false;
    let mut highest_paired_rank = None;

    for (&rank, &count) in &rank_counts {
        match count {
            2 => {
                pairs += 1;
                if highest_paired_rank.is_none() || rank > highest_paired_rank.unwrap() {
                    highest_paired_rank = Some(rank);
                }
            }
            3 => trips = true,
            4 => {
                return HandEvaluation {
                    rank: HandRank::FourOfAKind,
                    kickers: vec![rank],
                    description: format!("Four of a kind, {}", rank_name(rank)),
                }
            }
            _ => {}
        }
    }

    if trips {
        let trip_rank = rank_counts
            .iter()
            .find(|(_, &c)| c == 3)
            .map(|(&r, _)| r)
            .unwrap();
        return HandEvaluation {
            rank: HandRank::ThreeOfAKind,
            kickers: vec![trip_rank],
            description: format!("Three of a kind, {}", rank_name(trip_rank)),
        };
    }

    if pairs >= 2 {
        return HandEvaluation {
            rank: HandRank::TwoPair,
            kickers: highest_paired_rank.into_iter().collect(),
            description: "Two pair".to_string(),
        };
    }

    if pairs == 1 {
        let pair_rank = highest_paired_rank.unwrap();
        return HandEvaluation {
            rank: HandRank::Pair,
            kickers: vec![pair_rank],
            description: format!("Pair of {}", rank_name(pair_rank)),
        };
    }

    let mut ranks: Vec<Rank> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| b.cmp(a));

    let high_card = ranks[0];
    HandEvaluation {
        rank: HandRank::HighCard,
        kickers: ranks,
        description: format!("{} high", rank_name(high_card)),
    }
}

fn evaluate_five(cards: &[Card]) -> HandEvaluation {
    let mut rank_counts: HashMap<Rank, u8> = HashMap::new();
    let mut suit_counts: HashMap<super::deck::Suit, u8> = HashMap::new();

    for card in cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
        *suit_counts.entry(card.suit).or_insert(0) += 1;
    }

    let is_flush = suit_counts.values().any(|&c| c >= 5);

    let mut ranks: Vec<Rank> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| b.cmp(a));
    ranks.dedup();

    let straight_high = check_straight(&ranks);

    // Check for straight flush
    if is_flush {
        if let Some(high) = straight_high {
            return HandEvaluation {
                rank: HandRank::StraightFlush,
                kickers: vec![high],
                description: format!("{} high straight flush", rank_name(high)),
            };
        }
    }

    // Four of a kind
    if let Some((&rank, _)) = rank_counts.iter().find(|(_, &c)| c == 4) {
        return HandEvaluation {
            rank: HandRank::FourOfAKind,
            kickers: std::iter::once(rank)
                .chain(ranks.iter().copied().filter(|&other| other != rank))
                .collect(),
            description: format!("Four of a kind, {}", rank_name(rank)),
        };
    }

    // Full house
    let trips = rank_counts.iter().find(|(_, &c)| c == 3).map(|(&r, _)| r);
    let pair = rank_counts.iter().find(|(_, &c)| c == 2).map(|(&r, _)| r);

    if let (Some(trips), Some(pair)) = (trips, pair) {
        return HandEvaluation {
            rank: HandRank::FullHouse,
            kickers: vec![trips, pair],
            description: format!(
                "Full house, {} full of {}",
                rank_name(trips),
                rank_name(pair)
            ),
        };
    }

    if is_flush {
        return HandEvaluation {
            rank: HandRank::Flush,
            kickers: ranks.clone(),
            description: format!("{} high flush", rank_name(ranks[0])),
        };
    }

    if let Some(high) = straight_high {
        return HandEvaluation {
            rank: HandRank::Straight,
            kickers: vec![high],
            description: format!("{} high straight", rank_name(high)),
        };
    }

    if let Some(trip_rank) = trips {
        return HandEvaluation {
            rank: HandRank::ThreeOfAKind,
            kickers: std::iter::once(trip_rank)
                .chain(ranks.iter().copied().filter(|&rank| rank != trip_rank))
                .collect(),
            description: format!("Three of a kind, {}", rank_name(trip_rank)),
        };
    }

    // Two pair
    let pairs: Vec<Rank> = rank_counts
        .iter()
        .filter(|(_, &c)| c == 2)
        .map(|(&r, _)| r)
        .collect();

    if pairs.len() >= 2 {
        let mut sorted_pairs = pairs.clone();
        sorted_pairs.sort_by(|a, b| b.cmp(a));
        let high_pair = sorted_pairs[0];
        let low_pair = sorted_pairs[1];
        sorted_pairs.extend(
            ranks
                .iter()
                .copied()
                .filter(|&rank| rank != high_pair && rank != low_pair),
        );
        return HandEvaluation {
            rank: HandRank::TwoPair,
            kickers: sorted_pairs,
            description: format!(
                "Two pair, {} and {}",
                rank_name(high_pair),
                rank_name(low_pair)
            ),
        };
    }

    // One pair
    if pairs.len() == 1 {
        return HandEvaluation {
            rank: HandRank::Pair,
            kickers: std::iter::once(pairs[0])
                .chain(ranks.iter().copied().filter(|&rank| rank != pairs[0]))
                .collect(),
            description: format!("Pair of {}", rank_name(pairs[0])),
        };
    }

    // High card
    HandEvaluation {
        rank: HandRank::HighCard,
        kickers: ranks,
        description: format!(
            "{} high",
            rank_name(cards.iter().map(|c| c.rank).max().unwrap())
        ),
    }
}

/// Returns the high card of the straight if one exists, None otherwise.
/// For a wheel (A-2-3-4-5), returns Five (not Ace) since it's the lowest straight.
fn check_straight(sorted_ranks: &[Rank]) -> Option<Rank> {
    if sorted_ranks.len() < 5 {
        return None;
    }

    // Check for wheel (A-2-3-4-5) - returns Five as high card
    let values: Vec<u8> = sorted_ranks.iter().map(|r| *r as u8).collect();
    if values.contains(&14)
        && values.contains(&2)
        && values.contains(&3)
        && values.contains(&4)
        && values.contains(&5)
    {
        return Some(Rank::Five);
    }

    // Check for regular straight
    for window in sorted_ranks.windows(5) {
        let vals: Vec<u8> = window.iter().map(|r| *r as u8).collect();
        if vals[0] as i8 - vals[4] as i8 == 4 {
            return Some(window[0]); // Highest card in the window
        }
    }

    None
}

fn combinations(cards: &[Card], k: usize) -> Vec<Vec<Card>> {
    if k == 0 {
        return vec![vec![]];
    }
    if cards.len() < k {
        return vec![];
    }

    let mut result = Vec::new();
    for (i, &card) in cards.iter().enumerate() {
        let rest = &cards[i + 1..];
        for mut combo in combinations(rest, k - 1) {
            combo.insert(0, card);
            result.push(combo);
        }
    }
    result
}

fn rank_name(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "twos",
        Rank::Three => "threes",
        Rank::Four => "fours",
        Rank::Five => "fives",
        Rank::Six => "sixes",
        Rank::Seven => "sevens",
        Rank::Eight => "eights",
        Rank::Nine => "nines",
        Rank::Ten => "tens",
        Rank::Jack => "jacks",
        Rank::Queen => "queens",
        Rank::King => "kings",
        Rank::Ace => "aces",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::Suit;

    #[test]
    fn test_pair() {
        let cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Jack, Suit::Spades),
        ];
        let eval = evaluate_five(&cards);
        assert_eq!(eval.rank, HandRank::Pair);
    }

    #[test]
    fn test_flush() {
        let cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Nine, Suit::Spades),
        ];
        let eval = evaluate_five(&cards);
        assert_eq!(eval.rank, HandRank::Flush);
    }

    #[test]
    fn test_straight() {
        let cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Spades),
        ];
        let eval = evaluate_five(&cards);
        assert_eq!(eval.rank, HandRank::Straight);
    }

    #[test]
    fn test_wheel_straight() {
        // Wheel: A-2-3-4-5 should have Five as high card
        let wheel = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Three, Suit::Diamonds),
            Card::new(Rank::Four, Suit::Clubs),
            Card::new(Rank::Five, Suit::Spades),
        ];
        let wheel_eval = evaluate_five(&wheel);
        assert_eq!(wheel_eval.rank, HandRank::Straight);
        assert_eq!(wheel_eval.kickers[0], Rank::Five);

        // Six-high straight: 2-3-4-5-6 should beat wheel
        let six_high = vec![
            Card::new(Rank::Two, Suit::Spades),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Six, Suit::Spades),
        ];
        let six_eval = evaluate_five(&six_high);
        assert_eq!(six_eval.rank, HandRank::Straight);
        assert_eq!(six_eval.kickers[0], Rank::Six);

        // Six-high should beat wheel (Five < Six)
        assert!(six_eval.kickers > wheel_eval.kickers);
    }

    #[test]
    fn best_five_identifies_the_cards_that_play() {
        let hole = [
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
        ];
        let board = [
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Queen, Suit::Diamonds),
            Card::new(Rank::Jack, Suit::Clubs),
            Card::new(Rank::Ten, Suit::Spades),
        ];

        let (evaluation, cards) = evaluate_best_five(&hole, &board).unwrap();
        assert_eq!(evaluation.rank, HandRank::Straight);
        assert_eq!(cards, board);
    }

    #[test]
    fn best_five_two_pair_uses_the_ace_from_the_played_hand() {
        use Rank::*;
        use Suit::*;
        let hole = [Card::new(Jack, Spades), Card::new(Two, Hearts)];
        let board = [
            Card::new(Four, Spades),
            Card::new(Two, Diamonds),
            Card::new(Ace, Spades),
            Card::new(Six, Diamonds),
            Card::new(Jack, Hearts),
        ];
        let (evaluation, cards) = evaluate_best_five(&hole, &board).unwrap();
        assert_eq!(evaluation.rank, HandRank::TwoPair);
        assert_eq!(evaluation.kickers, [Jack, Two, Ace]);
        assert_eq!(cards, [hole[0], hole[1], board[1], board[2], board[4]]);
        // Selection strength must not depend on deal/enumeration order.
        let mut reversed = board;
        reversed.reverse();
        let (other, other_cards) = evaluate_best_five(&hole, &reversed).unwrap();
        assert_eq!(evaluation, other);
        assert!(cards.iter().all(|card| other_cards.contains(card)));
    }

    #[test]
    fn complete_kicker_vectors_break_every_duplicate_rank_tie() {
        use Rank::*;
        let cases = [
            (
                HandRank::Pair,
                [Jack, Jack, Ace, King, Nine],
                [Jack, Jack, Ace, King, Eight],
                vec![Jack, Ace, King, Nine],
            ),
            (
                HandRank::TwoPair,
                [Jack, Jack, Two, Two, Ace],
                [Jack, Jack, Two, Two, King],
                vec![Jack, Two, Ace],
            ),
            (
                HandRank::ThreeOfAKind,
                [Jack, Jack, Jack, Ace, King],
                [Jack, Jack, Jack, Ace, Queen],
                vec![Jack, Ace, King],
            ),
            (
                HandRank::FourOfAKind,
                [Jack, Jack, Jack, Jack, Ace],
                [Jack, Jack, Jack, Jack, King],
                vec![Jack, Ace],
            ),
        ];
        for (category, stronger, weaker, expected) in cases {
            let make_cards = |ranks: [Rank; 5]| {
                let suits = [
                    Suit::Spades,
                    Suit::Hearts,
                    Suit::Diamonds,
                    Suit::Clubs,
                    Suit::Spades,
                ];
                std::array::from_fn::<_, 5, _>(|i| Card::new(ranks[i], suits[i]))
            };
            let strong_cards = make_cards(stronger);
            let weak_cards = make_cards(weaker);
            let strong = evaluate_hand(&strong_cards[..2], &strong_cards[2..]);
            let weak = evaluate_hand(&weak_cards[..2], &weak_cards[2..]);
            assert_eq!(strong.rank, category);
            assert_eq!(weak.rank, category);
            assert_eq!(strong.kickers, expected);
            assert!(strong.kickers > weak.kickers, "{category:?}");
        }
    }

    #[test]
    fn board_two_pair_with_ace_kicker_still_ties_unplayed_hole_cards() {
        use Rank::*;
        use Suit::*;
        let board = [
            (Jack, Spades),
            (Jack, Hearts),
            (Two, Spades),
            (Two, Hearts),
            (Ace, Clubs),
        ]
        .map(|(rank, suit)| Card::new(rank, suit));
        for hole in [
            [Card::new(Four, Clubs), Card::new(Five, Diamonds)],
            [Card::new(Six, Clubs), Card::new(Seven, Diamonds)],
        ] {
            let (evaluation, cards) = evaluate_best_five(&hole, &board).unwrap();
            assert_eq!(evaluation.kickers, [Jack, Two, Ace]);
            assert_eq!(cards, board);
        }
    }

    #[test]
    fn test_straight_flush() {
        // Royal flush: A♠ K♠ Q♠ J♠ T♠
        let royal = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
        ];
        let eval = evaluate_five(&royal);
        assert_eq!(eval.rank, HandRank::StraightFlush);
        assert_eq!(eval.kickers[0], Rank::Ace);
    }

    #[test]
    fn test_straight_with_mixed_suits_is_not_straight_flush() {
        // A♠ K♠ Q♠ J♠ T♥ - straight but only 4 same suit (not flush, not straight flush)
        let cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Hearts), // Different suit
        ];
        let eval = evaluate_five(&cards);
        // This is just a straight, not a flush (only 4 spades)
        assert_eq!(eval.rank, HandRank::Straight);
    }

    #[test]
    fn test_flush_without_straight() {
        // A♠ K♠ Q♠ J♠ 9♠ - flush but not a straight (gap at T)
        let cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Nine, Suit::Spades),
        ];
        let eval = evaluate_five(&cards);
        assert_eq!(eval.rank, HandRank::Flush);
    }
}
