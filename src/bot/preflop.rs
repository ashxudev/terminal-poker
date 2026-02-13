use crate::game::deck::{Card, Rank, Suit};

/// Preflop hand tier for heads-up play, ordered weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreflopTier {
    Trash,
    Marginal,
    Playable,
    Strong,
    Premium,
}

impl PreflopTier {
    /// Returns the base win-rate estimate for this tier.
    pub fn base_strength(self) -> f64 {
        match self {
            PreflopTier::Premium => 0.90,
            PreflopTier::Strong => 0.75,
            PreflopTier::Playable => 0.60,
            PreflopTier::Marginal => 0.45,
            PreflopTier::Trash => 0.25,
        }
    }
}

/// Internal tier encoding: 1=Premium, 2=Strong, 3=Playable, 4=Marginal, 5=Trash
const P: u8 = 1;
const S: u8 = 2;
const L: u8 = 3;
const M: u8 = 4;
const T: u8 = 5;

/// Pair tiers indexed by `(rank as u8 - 2)`.
/// Index: 0=22, 1=33, 2=44, 3=55, 4=66, 5=77, 6=88, 7=99, 8=TT, 9=JJ, 10=QQ, 11=KK, 12=AA
#[rustfmt::skip]
const PAIR_TIER: [u8; 13] = [
    M, M, M, M, L, L, L, L, S, S, P, P, P,
];

/// Suited hand tiers: SUITED[low_rank_idx][high_rank_idx].
/// Only entries where high > low are used. Unused positions are 0.
/// Indices: 0=2, 1=3, 2=4, 3=5, 4=6, 5=7, 6=8, 7=9, 8=T, 9=J, 10=Q, 11=K, 12=A
#[rustfmt::skip]
const SUITED: [[u8; 13]; 13] = [
    //  2  3  4  5  6  7  8  9  T  J  Q  K  A
    [0, T, T, T, T, T, T, T, T, T, T, M, L], // low=2
    [0, 0, M, T, T, T, T, T, T, T, T, M, L], // low=3
    [0, 0, 0, M, M, T, T, T, T, T, T, M, L], // low=4
    [0, 0, 0, 0, M, M, M, T, T, T, T, M, L], // low=5
    [0, 0, 0, 0, 0, M, L, M, T, T, T, M, L], // low=6
    [0, 0, 0, 0, 0, 0, L, L, M, T, T, M, L], // low=7
    [0, 0, 0, 0, 0, 0, 0, L, L, M, M, M, L], // low=8
    [0, 0, 0, 0, 0, 0, 0, 0, L, L, M, L, L], // low=9
    [0, 0, 0, 0, 0, 0, 0, 0, 0, L, L, L, S], // low=T
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, L, S, S], // low=J
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, S, S], // low=Q
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, P], // low=K
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // low=A (no suited hand with higher rank)
];

/// Offsuit hand tiers: OFFSUIT[high_rank_idx][low_rank_idx].
/// Only entries where high > low are used. Unused positions are 0.
/// Indices: 0=2, 1=3, 2=4, 3=5, 4=6, 5=7, 6=8, 7=9, 8=T, 9=J, 10=Q, 11=K, 12=A
#[rustfmt::skip]
const OFFSUIT: [[u8; 13]; 13] = [
    //  2  3  4  5  6  7  8  9  T  J  Q  K  A
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=2 (no offsuit hand with lower rank)
    [T, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=3
    [T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=4
    [T, T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=5
    [T, T, T, M, 0, 0, 0, 0, 0, 0, 0, 0, 0], // high=6
    [T, T, T, T, M, 0, 0, 0, 0, 0, 0, 0, 0], // high=7
    [T, T, T, T, T, M, 0, 0, 0, 0, 0, 0, 0], // high=8
    [T, T, T, T, T, T, M, 0, 0, 0, 0, 0, 0], // high=9
    [T, T, T, T, T, T, T, M, 0, 0, 0, 0, 0], // high=T
    [T, T, T, T, T, T, T, T, M, 0, 0, 0, 0], // high=J
    [T, T, T, T, T, T, T, T, M, M, 0, 0, 0], // high=Q
    [T, T, T, T, T, T, T, T, M, M, L, 0, 0], // high=K
    [T, T, T, M, M, M, M, M, L, L, S, P, 0], // high=A
];

fn tier_from_code(code: u8) -> PreflopTier {
    match code {
        P => PreflopTier::Premium,
        S => PreflopTier::Strong,
        L => PreflopTier::Playable,
        M => PreflopTier::Marginal,
        _ => PreflopTier::Trash,
    }
}

fn rank_index(rank: Rank) -> usize {
    (rank as u8 - 2) as usize
}

/// Classify a two-card starting hand into a preflop tier.
///
/// # Panics
/// Panics if `cards` does not contain exactly 2 cards.
pub fn classify_preflop(cards: &[Card]) -> PreflopTier {
    assert_eq!(cards.len(), 2, "classify_preflop requires exactly 2 cards");

    let r0 = cards[0].rank;
    let r1 = cards[1].rank;
    let suited = cards[0].suit == cards[1].suit;

    if r0 == r1 {
        return tier_from_code(PAIR_TIER[rank_index(r0)]);
    }

    let (high, low) = if r0 > r1 { (r0, r1) } else { (r1, r0) };
    let hi = rank_index(high);
    let lo = rank_index(low);

    let code = if suited {
        SUITED[lo][hi]
    } else {
        OFFSUIT[hi][lo]
    };

    tier_from_code(code)
}

/// Return estimated preflop hand strength (0.0 to 1.0).
///
/// Combines the tier's base strength with a small kicker bonus
/// (up to +0.05) based on the high card rank within the tier.
pub fn preflop_strength(cards: &[Card]) -> f64 {
    let tier = classify_preflop(cards);
    let base = tier.base_strength();

    let high_rank = cards[0].rank.max(cards[1].rank);
    let low_rank = cards[0].rank.min(cards[1].rank);
    let kicker_bonus =
        (high_rank as u8 - 2) as f64 / 12.0 * 0.04 + (low_rank as u8 - 2) as f64 / 12.0 * 0.01;

    (base + kicker_bonus).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card::new(rank, suit)
    }

    fn hand(r0: Rank, s0: Suit, r1: Rank, s1: Suit) -> Vec<Card> {
        vec![card(r0, s0), card(r1, s1)]
    }

    // Suited helper: both spades
    fn suited(r0: Rank, r1: Rank) -> Vec<Card> {
        hand(r0, Suit::Spades, r1, Suit::Spades)
    }

    // Offsuit helper: first spades, second hearts
    fn offsuit(r0: Rank, r1: Rank) -> Vec<Card> {
        hand(r0, Suit::Spades, r1, Suit::Hearts)
    }

    // Pair helper: different suits
    fn pair(r: Rank) -> Vec<Card> {
        hand(r, Suit::Spades, r, Suit::Hearts)
    }

    #[test]
    fn test_premium_hands() {
        assert_eq!(classify_preflop(&pair(Rank::Ace)), PreflopTier::Premium);
        assert_eq!(classify_preflop(&pair(Rank::King)), PreflopTier::Premium);
        assert_eq!(
            classify_preflop(&suited(Rank::Ace, Rank::King)),
            PreflopTier::Premium
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Ace, Rank::King)),
            PreflopTier::Premium
        );
    }

    #[test]
    fn test_strong_hands() {
        assert_eq!(classify_preflop(&pair(Rank::Jack)), PreflopTier::Strong);
        assert_eq!(
            classify_preflop(&suited(Rank::Ace, Rank::Queen)),
            PreflopTier::Strong
        );
        assert_eq!(
            classify_preflop(&suited(Rank::King, Rank::Queen)),
            PreflopTier::Strong
        );
    }

    #[test]
    fn test_playable_hands() {
        assert_eq!(classify_preflop(&pair(Rank::Nine)), PreflopTier::Playable);
        assert_eq!(
            classify_preflop(&suited(Rank::Ace, Rank::Two)),
            PreflopTier::Playable
        );
        assert_eq!(
            classify_preflop(&suited(Rank::Ten, Rank::Eight)),
            PreflopTier::Playable
        );
        assert_eq!(
            classify_preflop(&suited(Rank::Nine, Rank::Seven)),
            PreflopTier::Playable
        );
        assert_eq!(
            classify_preflop(&suited(Rank::Eight, Rank::Six)),
            PreflopTier::Playable
        );
    }

    #[test]
    fn test_marginal_hands() {
        assert_eq!(classify_preflop(&pair(Rank::Two)), PreflopTier::Marginal);
        assert_eq!(
            classify_preflop(&offsuit(Rank::Ace, Rank::Five)),
            PreflopTier::Marginal
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Five, Rank::Four)),
            PreflopTier::Marginal
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Four, Rank::Three)),
            PreflopTier::Marginal
        );
    }

    #[test]
    fn test_trash_hands() {
        assert_eq!(
            classify_preflop(&offsuit(Rank::Seven, Rank::Two)),
            PreflopTier::Trash
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Ace, Rank::Four)),
            PreflopTier::Trash
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::King, Rank::Nine)),
            PreflopTier::Trash
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Queen, Rank::Nine)),
            PreflopTier::Trash
        );
        assert_eq!(
            classify_preflop(&offsuit(Rank::Three, Rank::Two)),
            PreflopTier::Trash
        );
    }

    #[test]
    fn test_strength_ordering() {
        let aa = preflop_strength(&pair(Rank::Ace));
        let aks = preflop_strength(&suited(Rank::Ace, Rank::King));
        let jj = preflop_strength(&pair(Rank::Jack));
        let nn = preflop_strength(&pair(Rank::Nine));
        let fives = preflop_strength(&pair(Rank::Five));
        let seven_two = preflop_strength(&offsuit(Rank::Seven, Rank::Two));

        assert!(
            aa > aks,
            "AA ({aa}) should beat AKs ({aks})"
        );
        assert!(
            aks > jj,
            "AKs ({aks}) should beat JJ ({jj})"
        );
        assert!(
            jj > nn,
            "JJ ({jj}) should beat 99 ({nn})"
        );
        assert!(
            nn > fives,
            "99 ({nn}) should beat 55 ({fives})"
        );
        assert!(
            fives > seven_two,
            "55 ({fives}) should beat 72o ({seven_two})"
        );
    }

    #[test]
    fn test_strength_capped_at_one() {
        let s = preflop_strength(&pair(Rank::Ace));
        assert!(s <= 1.0, "strength should not exceed 1.0, got {s}");
    }

    #[test]
    fn test_fixed_matrix_entries() {
        // These hands had lookup table errors that were corrected.
        // Verify they match the canonical matrix.
        assert_eq!(classify_preflop(&suited(Rank::Jack, Rank::Eight)), PreflopTier::Marginal);   // J8s
        assert_eq!(classify_preflop(&suited(Rank::Jack, Rank::Seven)), PreflopTier::Trash);      // J7s
        assert_eq!(classify_preflop(&suited(Rank::Ten, Rank::Seven)), PreflopTier::Marginal);    // T7s
        assert_eq!(classify_preflop(&suited(Rank::Ten, Rank::Six)), PreflopTier::Trash);         // T6s
        assert_eq!(classify_preflop(&suited(Rank::Nine, Rank::Six)), PreflopTier::Marginal);     // 96s
        assert_eq!(classify_preflop(&suited(Rank::Eight, Rank::Five)), PreflopTier::Marginal);   // 85s
        assert_eq!(classify_preflop(&offsuit(Rank::King, Rank::Ten)), PreflopTier::Marginal);    // KTo
    }

    #[test]
    fn test_card_order_does_not_matter() {
        // AKs should be the same regardless of card order
        let h1 = suited(Rank::Ace, Rank::King);
        let h2 = suited(Rank::King, Rank::Ace);
        assert_eq!(classify_preflop(&h1), classify_preflop(&h2));

        // 72o should be the same regardless of card order
        let h3 = offsuit(Rank::Seven, Rank::Two);
        let h4 = offsuit(Rank::Two, Rank::Seven);
        assert_eq!(classify_preflop(&h3), classify_preflop(&h4));
    }
}
