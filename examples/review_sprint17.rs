//! Reproducible production-renderer evidence over real authoritative hands.
use ratatui::{backend::TestBackend, Terminal};
use serde_json::{json, Value};
use std::{env, fs, path::Path};
use terminal_poker::{
    game::{
        actions::Action,
        command::SeatCommand,
        multiway::{MultiwayHand, MultiwayPhase},
        seat::{SeatId, TableSize},
    },
    protocol::{
        project_hand, HandId, ProjectionAudience, SnapshotEnvelope, TableId, PROTOCOL_VERSION,
    },
    ui::{
        ash_table::render_with_state,
        multiway_review::{MultiwayReviewView, ProtocolReviewMetadata, ShowdownStage},
    },
};

fn s(n: u8) -> SeatId {
    SeatId::new(n).unwrap()
}
fn hand() -> MultiwayHand {
    let mut h = MultiwayHand::new_seeded_for_review(
        TableSize::new(3).unwrap(),
        s(0),
        &[(s(0), 100), (s(1), 100), (s(2), 100)],
        31_415,
    )
    .unwrap();
    h.enable_paced_showdown();
    h
}
fn capture(
    out: &Path,
    h: &MultiwayHand,
    identity: &str,
    label: &str,
    revision: u64,
    stage: Option<ShowdownStage>,
    log: &[String],
) -> Result<Value, Box<dyn std::error::Error>> {
    let snapshot = SnapshotEnvelope {
        version: PROTOCOL_VERSION,
        table_id: TableId(17),
        hand_id: HandId(1),
        revision,
        snapshot: project_hand(h, HandId(1), ProjectionAudience::Player(s(0)))
            .map_err(|e| format!("projection failed: {e:?}"))?,
    };
    let review_seed = identity
        .strip_prefix("ONE-HOLE-SEED-")
        .and_then(|seed| seed.parse().ok())
        .unwrap_or(31_415);
    let view = MultiwayReviewView::from_projection(
        &snapshot,
        "sprint17-showdown",
        identity,
        review_seed,
        label,
        ProtocolReviewMetadata {
            version: PROTOCOL_VERSION,
            table_id: 17,
            hand_id: 1,
            revision,
            audience: "Player S0".into(),
            command_id: format!("review-{revision}"),
            outcome: "accepted".into(),
        },
        log.to_vec(),
    );
    for (width, height) in [(80, 30), (56, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| render_with_state(frame, &view, 0, None, stage))?;
        let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|cell|json!({"symbol":cell.symbol(),"foreground":format!("{:?}",cell.fg),"background":format!("{:?}",cell.bg),"modifiers":cell.modifier.bits()})).collect();
        let checkpoint = format!("{label}-{width}x{height}");
        fs::write(
            out.join("captures").join(format!("{checkpoint}.json")),
            serde_json::to_vec_pretty(
                &json!({"renderer":"terminal_poker::ui::ash_table::render_with_state","backend":"ratatui::backend::TestBackend","checkpoint":checkpoint,"width":width,"height":height,"cells":cells}),
            )?,
        )?;
    }
    Ok(
        json!({"identity":identity,"label":label,"revision":revision,"phase":h.phase.name(),"board":snapshot.snapshot.board,"pot":snapshot.snapshot.pot_total,"seats":snapshot.snapshot.seats.iter().map(|s|json!({"seat":s.seat,"stack":s.stack,"contribution":s.hand_contribution})).collect::<Vec<_>>(),"shown":snapshot.snapshot.shown,"mucked":snapshot.snapshot.mucked,"awards":snapshot.snapshot.awards,"events":log}),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arg = env::args().nth(1).ok_or("expected output directory")?;
    let out = Path::new(&arg);
    fs::create_dir_all(out.join("captures"))?;
    let mut ledger = Vec::new();
    let mut h = hand();
    let mut log = Vec::new();
    let mut rev = 0;
    ledger.push(capture(
        out,
        &h,
        "S17-ORDINARY-001",
        "01-deal",
        rev,
        None,
        &log,
    )?);
    let mut phase = h.phase;
    while let Some(actor) = h.to_act {
        let action =
            terminal_poker::network_client::passive_action(&h.legal_actions_for(actor).unwrap());
        log.push(format!(
            "S{} {action:?} on {}",
            actor.as_u8(),
            h.phase.name()
        ));
        h.apply_command(SeatCommand::new(actor, action))?;
        rev += 1;
        if h.phase != phase && h.showdown_progress.is_none() {
            phase = h.phase;
            let label = format!("02-{}", phase.name().to_lowercase());
            ledger.push(capture(
                out,
                &h,
                "S17-ORDINARY-001",
                &label,
                rev,
                None,
                &log,
            )?);
        }
    }
    ledger.push(capture(
        out,
        &h,
        "S17-ORDINARY-001",
        "03-first-show",
        rev,
        None,
        &log,
    )?);
    let mut step = 0;
    while h.advance_showdown() {
        rev += 1;
        step += 1;
        log.push(format!("Dealer showdown transition {step}"));
        let label = if h.phase == MultiwayPhase::Showdown {
            "06-winner".into()
        } else {
            format!("04-step-{step}")
        };
        ledger.push(capture(
            out,
            &h,
            "S17-ORDINARY-001",
            &label,
            rev,
            Some(ShowdownStage::Winners),
            &log,
        )?);
    }
    ledger.push(capture(
        out,
        &h,
        "S17-ORDINARY-001",
        "07-award",
        rev,
        Some(ShowdownStage::Award),
        &log,
    )?);
    assert_eq!(h.total_chips(), 300);
    assert_eq!(h.mucked_hands.len(), 2);

    let mut h = hand();
    let mut rev = 0;
    let mut log = Vec::new();
    while let Some(actor) = h.to_act {
        let action = Action::AllIn(h.legal_actions_for(actor).unwrap().all_in_to);
        log.push(format!("S{} {action:?}", actor.as_u8()));
        h.apply_command(SeatCommand::new(actor, action))?;
        rev += 1;
    }
    assert!(h.board.is_empty());
    assert_eq!(h.revealed_hands.len(), 3);
    ledger.push(capture(
        out,
        &h,
        "S17-ALLIN-001",
        "08-all-in-cards-up",
        rev,
        None,
        &log,
    )?);
    while h.advance_showdown() {
        rev += 1;
        log.push(format!("Dealer runout / {}", h.phase.name()));
    }
    ledger.push(capture(
        out,
        &h,
        "S17-ALLIN-001",
        "09-all-in-winner",
        rev,
        Some(ShowdownStage::Winners),
        &log,
    )?);
    assert_eq!(h.total_chips(), 300);

    let mut h = hand();
    let mut rev = 0;
    let mut log = Vec::new();
    while let Some(actor) = h.to_act {
        let action = Action::Fold;
        log.push(format!("S{} folds", actor.as_u8()));
        h.apply_command(SeatCommand::new(actor, action))?;
        rev += 1;
    }
    ledger.push(capture(
        out,
        &h,
        "S17-FOLD-001",
        "10-uncontested",
        rev,
        Some(ShowdownStage::Award),
        &log,
    )?);
    assert!(h.revealed_hands.is_empty());
    assert_eq!(h.phase, MultiwayPhase::HandComplete);
    // Additional post-review regression evidence: a real deal whose winner
    // plays exactly one hole card. Search is confined to deterministic review.
    for seed in 1..=1_000 {
        let mut candidate = MultiwayHand::new_seeded_for_review(
            TableSize::new(3)?,
            s(0),
            &[(s(0), 100), (s(1), 100), (s(2), 100)],
            seed,
        )?;
        while let Some(actor) = candidate.to_act {
            let action = terminal_poker::network_client::passive_action(
                &candidate.legal_actions_for(actor).unwrap(),
            );
            candidate.apply_command(SeatCommand::new(actor, action))?;
        }
        let winners = &candidate.awards[0].winners;
        if winners.len() != 1 {
            continue;
        }
        let winner = winners[0];
        let hole = &candidate.seat(winner).hole_cards;
        let (_, best) =
            terminal_poker::game::hand::evaluate_best_five(hole, &candidate.board).unwrap();
        if hole.iter().filter(|card| best.contains(card)).count() != 1 {
            continue;
        }
        ledger.push(capture(
            out,
            &candidate,
            &format!("ONE-HOLE-SEED-{seed}"),
            "11-one-playing-hole-card",
            0,
            Some(ShowdownStage::Winners),
            &[],
        )?);
        break;
    }
    assert!(ledger
        .iter()
        .any(|row| row["label"] == "11-one-playing-hole-card"));
    let mut uncalled = hand();
    let actor = uncalled.to_act.unwrap();
    let target = uncalled.legal_actions_for(actor).unwrap().all_in_to;
    uncalled.apply_command(SeatCommand::new(actor, Action::AllIn(target)))?;
    while let Some(actor) = uncalled.to_act {
        uncalled.apply_command(SeatCommand::new(actor, Action::Fold))?;
    }
    assert!(uncalled.board.is_empty());
    assert!(uncalled.revealed_hands.is_empty());
    ledger.push(capture(
        out,
        &uncalled,
        "UNCALLED-SHOVE",
        "12-uncalled-no-runout",
        3,
        Some(ShowdownStage::Award),
        &[],
    )?);
    fs::write(
        out.join("evidence.json"),
        serde_json::to_vec_pretty(
            &json!({"build":"sprint17-showdown","seed":31415,"initial_stacks":[100,100,100],"blinds":[1,2],"ledger":ledger}),
        )?,
    )?;
    println!("SPRINT17_CAPTURE_PASS");
    Ok(())
}
