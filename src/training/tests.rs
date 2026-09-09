use std::collections::{BTreeMap, HashSet};

use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::authorized_table::GuestSessionId;
use crate::game::actions::Action;
use crate::game::deck::{Card, Rank, Suit};
use crate::game::multiway::{MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::lobby::{PublicTableConfig, TableVisibility};
use crate::protocol::{CommandEnvelope, CommandOutcome};
use crate::table_registry::TableRegistry;

use super::*;

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("test seat is valid")
}

fn card(rank: Rank, suit: Suit) -> Card {
    Card::new(rank, suit)
}

fn heads_up_assignments() -> (BTreeMap<SeatId, [Card; 2]>, [Card; 5]) {
    let holes = BTreeMap::from([
        (
            seat(0),
            [card(Rank::Ace, Suit::Spades), card(Rank::Ace, Suit::Hearts)],
        ),
        (
            seat(1),
            [
                card(Rank::King, Suit::Clubs),
                card(Rank::King, Suit::Diamonds),
            ],
        ),
    ]);
    let board = [
        card(Rank::Two, Suit::Clubs),
        card(Rank::Three, Suit::Diamonds),
        card(Rank::Four, Suit::Spades),
        card(Rank::Five, Suit::Hearts),
        card(Rank::Six, Suit::Clubs),
    ];
    (holes, board)
}

fn check_call_policies() -> BTreeMap<SeatId, Box<dyn Policy>> {
    BTreeMap::from([
        (
            seat(0),
            Box::<CheckCallPolicy>::default() as Box<dyn Policy>,
        ),
        (
            seat(1),
            Box::<CheckCallPolicy>::default() as Box<dyn Policy>,
        ),
    ])
}

#[test]
fn deal_plan_rejects_wrong_counts_duplicates_and_inconsistent_assignments() {
    assert!(matches!(
        DealPlanV1::new(Vec::new()),
        Err(DealPlanError::WrongCardCount(0))
    ));
    let mut duplicate = DealPlanV1::seeded(7).cards;
    duplicate[1] = duplicate[0];
    assert!(matches!(
        DealPlanV1::new(duplicate),
        Err(DealPlanError::DuplicateCard(_))
    ));

    let (mut holes, board) = heads_up_assignments();
    holes.remove(&seat(1));
    assert_eq!(
        DealPlanV1::from_assignments(
            TableSize::new(2).unwrap(),
            seat(0),
            &[seat(0), seat(1)],
            &holes,
            board,
            9,
        ),
        Err(DealPlanError::HoleAssignmentMismatch)
    );
}

#[test]
fn assigned_private_cards_and_public_runout_follow_engine_deal_order() {
    let (holes, board) = heads_up_assignments();
    let plan = DealPlanV1::from_assignments(
        TableSize::new(2).unwrap(),
        seat(0),
        &[seat(0), seat(1)],
        &holes,
        board,
        11,
    )
    .unwrap();
    let hand = MultiwayHand::new_with_deck_for_training(
        TableSize::new(2).unwrap(),
        seat(0),
        &[(seat(0), 100), (seat(1), 100)],
        &[],
        plan.clone().into_deck().unwrap(),
    )
    .unwrap();
    assert_eq!(hand.seat(seat(0)).hole_cards, holes[&seat(0)]);
    assert_eq!(hand.seat(seat(1)).hole_cards, holes[&seat(1)]);

    let replacement = [
        card(Rank::Seven, Suit::Clubs),
        card(Rank::Eight, Suit::Diamonds),
        card(Rank::Nine, Suit::Spades),
        card(Rank::Ten, Suit::Hearts),
        card(Rank::Jack, Suit::Clubs),
    ];
    let branched = plan.branch_public_runout(2, replacement).unwrap();
    let mut arena = TrainingArena::new(ArenaConfig::heads_up(100), branched).unwrap();
    let episode = arena.run_to_terminal(&mut check_call_policies()).unwrap();
    assert_eq!(episode.safe_history.board, replacement);
    assert_eq!(episode.safe_history.publicly_revealed.len(), 2);
}

#[test]
fn weighted_ranges_remove_blocked_cards_and_use_separate_rng() {
    let blocked_combo = [card(Rank::Ace, Suit::Spades), card(Rank::Ace, Suit::Hearts)];
    let available_combo = [
        card(Rank::Queen, Suit::Spades),
        card(Rank::Queen, Suit::Hearts),
    ];
    let range = WeightedRangeV1::new(
        "two-combo",
        vec![
            WeightedHoleCombo {
                cards: blocked_combo,
                weight: 100,
            },
            WeightedHoleCombo {
                cards: available_combo,
                weight: 1,
            },
        ],
    )
    .unwrap();
    let blocked = HashSet::from(blocked_combo);
    let mut rng = StdRng::seed_from_u64(99);
    assert_eq!(
        range.sample_available(&blocked, &mut rng).unwrap(),
        available_combo
    );
}

#[test]
fn observation_contains_own_cards_and_public_history_but_no_hidden_deal_data() {
    let (holes, board) = heads_up_assignments();
    let plan = DealPlanV1::from_assignments(
        TableSize::new(2).unwrap(),
        seat(0),
        &[seat(0), seat(1)],
        &holes,
        board,
        17,
    )
    .unwrap();
    let mut arena = TrainingArena::new(ArenaConfig::heads_up(100), plan).unwrap();
    let first = arena.current_observation().unwrap().unwrap();
    assert_eq!(first.acting_seat, seat(0));
    assert_eq!(first.hole_cards, holes[&seat(0)]);
    assert!(first.action_history.is_empty());
    let json = serde_json::to_string(&first).unwrap();
    assert!(json.contains("Ace"));
    assert!(!json.contains("King"));
    for forbidden in [
        "deck",
        "seed",
        "rng",
        "guest",
        "session",
        "credential",
        "future",
        "range",
    ] {
        assert!(!json.to_ascii_lowercase().contains(forbidden));
    }

    arena.step(PolicyActionV1::Call).unwrap();
    let second = arena.current_observation().unwrap().unwrap();
    assert_eq!(second.action_history.len(), 1);
    assert_eq!(second.action_history[0].action, Action::Call(1));
}

#[test]
fn every_exposed_abstract_action_maps_to_an_accepted_authoritative_command() {
    let baseline = TrainingArena::new(ArenaConfig::heads_up(100), DealPlanV1::seeded(31))
        .unwrap()
        .current_observation()
        .unwrap()
        .unwrap();
    let legal = legal_policy_actions(&baseline);
    assert!(!legal.contains(&PolicyActionV1::Check));
    assert!(legal.contains(&PolicyActionV1::Call));
    assert!(legal.contains(&PolicyActionV1::BetRaisePot));
    assert!(legal.contains(&PolicyActionV1::AllIn));

    for action in legal {
        let mut arena =
            TrainingArena::new(ArenaConfig::heads_up(100), DealPlanV1::seeded(31)).unwrap();
        let decision = arena.step(action).unwrap();
        assert_eq!(decision.policy_action, action);
        assert_eq!(decision.accepted_event.revision, 1);
    }
}

#[test]
fn seeded_random_legal_policies_terminate_without_authority_rejection() {
    for seed in 0..24 {
        let mut arena =
            TrainingArena::new(ArenaConfig::heads_up(100), DealPlanV1::seeded(seed)).unwrap();
        let mut policies: BTreeMap<SeatId, Box<dyn Policy>> = BTreeMap::from([
            (
                seat(0),
                Box::new(RandomLegalPolicy::seeded(seed + 100)) as Box<dyn Policy>,
            ),
            (
                seat(1),
                Box::new(RandomLegalPolicy::seeded(seed + 200)) as Box<dyn Policy>,
            ),
        ]);
        let episode = arena.run_to_terminal(&mut policies).unwrap();
        assert!(matches!(
            episode.terminal_public_snapshot.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ));
        assert_eq!(
            episode
                .chip_deltas
                .iter()
                .map(|(_, delta)| delta)
                .sum::<i64>(),
            0
        );
    }
}

#[test]
fn fast_arena_matches_registry_safe_history_and_registry_rolls_over() {
    const SEED: u64 = 73;
    let mut arena =
        TrainingArena::new(ArenaConfig::heads_up(100), DealPlanV1::seeded(SEED)).unwrap();
    let arena_episode = arena.run_to_terminal(&mut check_call_policies()).unwrap();

    let mut registry = TableRegistry::new(1).unwrap();
    let table = registry
        .create(
            PublicTableConfig {
                name: "Training conformance".to_string(),
                seats: TableSize::new(2).unwrap(),
                starting_stack: 100,
                min_players: 2,
                visibility: TableVisibility::Public,
                join_code: None,
            },
            Some(SEED),
        )
        .unwrap();
    let sessions = BTreeMap::from([
        (seat(0), GuestSessionId::new("training-seat-0").unwrap()),
        (seat(1), GuestSessionId::new("training-seat-1").unwrap()),
    ]);
    for (&seat, session) in &sessions {
        registry
            .join(session.clone(), table.table_id, Some(seat))
            .unwrap();
    }

    let hand_id = registry
        .route_for_session(&sessions[&seat(0)])
        .unwrap()
        .hand_id;
    let mut accepted_events = Vec::new();
    loop {
        let sample_route = registry.route_for_session(&sessions[&seat(0)]).unwrap();
        let public_progress = sample_route
            .handle
            .snapshot(sessions[&seat(0)].clone())
            .unwrap();
        let Some(actor) = public_progress.snapshot.to_act else {
            if public_progress.snapshot.showdown.is_some() {
                std::thread::sleep(std::time::Duration::from_millis(20));
                continue;
            }
            break;
        };
        let actor_session = sessions[&actor].clone();
        let route = registry.route_for_session(&actor_session).unwrap();
        let snapshot = route.handle.snapshot(actor_session.clone()).unwrap();
        let observation =
            PolicyObservationV1::from_authorized(&snapshot, &accepted_events).unwrap();
        let policy_action = if observation.legal_actions.can_check {
            PolicyActionV1::Check
        } else {
            PolicyActionV1::Call
        };
        let action = map_policy_action(&observation, policy_action).unwrap();
        let response = route
            .handle
            .submit(
                actor_session,
                CommandEnvelope::act_for_hand(
                    format!("registry-train-{}", snapshot.revision + 1),
                    table.table_id,
                    hand_id,
                    snapshot.revision,
                    actor,
                    action,
                ),
            )
            .unwrap();
        let CommandOutcome::Accepted { event } = response.receipt.outcome else {
            panic!("mapped conformance action must be accepted");
        };
        accepted_events.push(event);
    }

    registry
        .finalize_safe_history(table.table_id, hand_id)
        .unwrap();
    assert_eq!(registry.safe_history_count(), 1);
    assert_eq!(
        registry.safe_histories().last().unwrap(),
        &arena_episode.safe_history
    );
    let successor = registry
        .rollover_completed_hand(table.table_id, hand_id)
        .unwrap()
        .expect("two funded seats start a successor hand");
    assert!(successor > hand_id);
}

#[test]
fn duplicate_fixture_replays_one_deal_with_policy_seats_exchanged() {
    let plan = DealPlanV1::seeded(121);
    let mut seat_pairs = HashSet::new();
    for exchange in [false, true] {
        let mut arena = TrainingArena::new(ArenaConfig::heads_up(100), plan.clone()).unwrap();
        let mut policies: BTreeMap<SeatId, Box<dyn Policy>> = if exchange {
            BTreeMap::from([
                (
                    seat(0),
                    Box::<CheckCallPolicy>::default() as Box<dyn Policy>,
                ),
                (
                    seat(1),
                    Box::new(RandomLegalPolicy::seeded(8)) as Box<dyn Policy>,
                ),
            ])
        } else {
            BTreeMap::from([
                (
                    seat(0),
                    Box::new(RandomLegalPolicy::seeded(8)) as Box<dyn Policy>,
                ),
                (
                    seat(1),
                    Box::<CheckCallPolicy>::default() as Box<dyn Policy>,
                ),
            ])
        };
        let initial = arena.current_observation().unwrap().unwrap();
        seat_pairs.insert((initial.acting_seat, initial.hole_cards));
        arena.run_to_terminal(&mut policies).unwrap();
    }
    assert_eq!(
        seat_pairs.len(),
        1,
        "the cloned deal is exact across seat exchange"
    );
}
