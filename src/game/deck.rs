use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    pub fn symbol(&self) -> &'static str {
        match self {
            Suit::Spades => "♠\u{FE0E}",
            Suit::Hearts => "♥\u{FE0E}",
            Suit::Diamonds => "♦\u{FE0E}",
            Suit::Clubs => "♣\u{FE0E}",
        }
    }

    pub fn is_red(&self) -> bool {
        matches!(self, Suit::Hearts | Suit::Diamonds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    pub fn symbol(&self) -> &'static str {
        match self {
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
        }
    }

    pub const ALL: [Rank; 13] = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank.symbol(), self.suit.symbol())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deck {
    cards: Vec<Card>,
    index: usize,
}

impl Deck {
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(52);
        for suit in [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
            for rank in Rank::ALL {
                cards.push(Card::new(rank, suit));
            }
        }
        Self { cards, index: 0 }
    }

    pub fn shuffle_with<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        self.cards.shuffle(rng);
        self.index = 0;
    }

    pub fn deal(&mut self) -> Option<Card> {
        if self.index < self.cards.len() {
            let card = self.cards[self.index];
            self.index += 1;
            Some(card)
        } else {
            None
        }
    }

    pub fn deal_n(&mut self, n: usize) -> Vec<Card> {
        (0..n).filter_map(|_| self.deal()).collect()
    }

    /// Constructs a deck from an already validated deal order.
    ///
    /// This is crate-private so production callers cannot replace the
    /// authoritative entropy source. The training module validates card count
    /// and uniqueness before crossing this boundary.
    pub(crate) fn from_ordered_cards_for_training(cards: Vec<Card>) -> Self {
        debug_assert_eq!(cards.len(), 52);
        Self { cards, index: 0 }
    }
}

/// Authoritative shuffle boundary.
///
/// Production construction seeds from operating-system-backed entropy. The
/// deterministic constructor exists for tests and review fixtures only; the
/// seed is intentionally not retained or exposed.
#[derive(Debug, Clone)]
pub struct ShuffleSource {
    rng: StdRng,
}

impl ShuffleSource {
    pub fn production() -> Self {
        Self {
            rng: StdRng::from_entropy(),
        }
    }

    pub fn deterministic_for_review(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn shuffle(&mut self, deck: &mut Deck) {
        deck.shuffle_with(&mut self.rng);
    }
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deck_has_52_cards() {
        let mut deck = Deck::new();
        let cards: Vec<_> = (0..52).filter_map(|_| deck.deal()).collect();
        assert_eq!(cards.len(), 52);
        assert!(deck.deal().is_none());
    }

    #[test]
    fn test_shuffle_resets_index() {
        let mut deck = Deck::new();
        deck.deal();
        deck.deal();
        let mut source = ShuffleSource::deterministic_for_review(7);
        source.shuffle(&mut deck);
        let cards: Vec<_> = (0..52).filter_map(|_| deck.deal()).collect();
        assert_eq!(cards.len(), 52);
    }

    #[test]
    fn same_review_seed_produces_same_deck_order() {
        let mut first = Deck::new();
        let mut second = Deck::new();
        ShuffleSource::deterministic_for_review(42).shuffle(&mut first);
        ShuffleSource::deterministic_for_review(42).shuffle(&mut second);

        assert_eq!(first, second);
    }

    #[test]
    fn different_review_seeds_produce_different_deck_orders() {
        let mut first = Deck::new();
        let mut second = Deck::new();
        ShuffleSource::deterministic_for_review(42).shuffle(&mut first);
        ShuffleSource::deterministic_for_review(43).shuffle(&mut second);

        assert_ne!(first, second);
    }
}
