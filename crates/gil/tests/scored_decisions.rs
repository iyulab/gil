//! The decisions of a judge outside Gil, read from settled records (experimental): the record carries the judge's
//! score beside the value it suggested, and `scored_decisions` turns the settled ones into what
//! `select_score_threshold` takes.

use gil::decider::Source;
use gil::replay::{Choice, ScoredDecision, scored_decisions, select_score_threshold};
use gil::schema::{Record, SettlementKind};

/// A record of `team` settled with `shown` in view, from `source`, scored `score`, settled as `kind` to `settled`.
fn line(
    id: &str,
    at: i64,
    source: Source,
    score: Option<f64>,
    shown: &str,
    kind: SettlementKind,
    settled: &str,
) -> Record {
    let mut record = Record {
        id: id.to_owned(),
        settled_at: at,
        ..Record::default()
    };
    record.values.insert("team".to_owned(), settled.to_owned());
    record.settlements.insert("team".to_owned(), kind);
    record.suggested.insert("team".to_owned(), shown.to_owned());
    record.sources.insert("team".to_owned(), source);
    if let Some(score) = score {
        record.scores.insert("team".to_owned(), score);
    }
    record
}

#[test]
fn a_record_gives_a_decision_when_a_judge_answered_and_a_person_settled_it() {
    let accepted = line(
        "a",
        1,
        Source::External,
        Some(0.9),
        "network",
        SettlementKind::Accept,
        "network",
    );
    let corrected = line(
        "b",
        2,
        Source::External,
        Some(0.4),
        "desk",
        SettlementKind::Correct,
        "network",
    );
    let decisions = scored_decisions(&[accepted, corrected], "team");
    assert_eq!(
        decisions,
        vec![
            ScoredDecision::new("a", 1, Some(0.9), true),
            ScoredDecision::new("b", 2, Some(0.4), false),
        ]
    );
}

#[test]
fn a_check_counts_as_one_over_the_probability_it_was_drawn_with() {
    let mut checked = line(
        "c",
        3,
        Source::External,
        Some(0.7),
        "network",
        SettlementKind::Check,
        "network",
    );
    checked.probabilities.insert("team".to_owned(), 0.25);
    let decisions = scored_decisions(&[checked.clone()], "team");
    assert_eq!(
        decisions,
        vec![ScoredDecision::new("c", 3, Some(0.7), true).weighted(4.0)]
    );

    // A check with no probability is taken once; one with a probability that cannot be one is left out.
    checked.probabilities.clear();
    assert_eq!(scored_decisions(&[checked.clone()], "team")[0].weight, 1.0);
    checked.probabilities.insert("team".to_owned(), 0.0);
    assert!(scored_decisions(&[checked], "team").is_empty());
}

#[test]
fn what_the_judge_got_right_or_wrong_is_not_known_for_the_rest() {
    let model = Source::External;
    let rest = [
        // another layer answered
        line(
            "k",
            1,
            Source::KeyMemory,
            Some(0.9),
            "a",
            SettlementKind::Accept,
            "a",
        ),
        // no score kept
        line("n", 2, model, None, "a", SettlementKind::Accept, "a"),
        // the answer was turned down, a value was entered with nothing in view, the host filled it, or the suggestion was
        // another's kept
        line("r", 3, model, Some(0.9), "a", SettlementKind::Reject, "a"),
        line("e", 4, model, Some(0.9), "a", SettlementKind::Enter, "a"),
        line("f", 5, model, Some(0.9), "a", SettlementKind::Fill, "a"),
        line(
            "o",
            6,
            model,
            Some(0.9),
            "a",
            SettlementKind::AcceptOther,
            "a",
        ),
        // a score that is not a number
        line(
            "x",
            7,
            model,
            Some(f64::NAN),
            "a",
            SettlementKind::Accept,
            "a",
        ),
    ];
    assert!(scored_decisions(&rest, "team").is_empty());
    // another attribute has no decisions here
    let one = line("a", 1, model, Some(0.9), "a", SettlementKind::Accept, "a");
    assert!(scored_decisions(&[one], "owner").is_empty());
}

#[test]
fn the_decisions_a_judge_made_choose_its_threshold_from_the_records_alone() {
    // A judge whose score is the chance it is right, over 4,000 records settled one tick apart: every fifth one is a check
    // drawn with probability 0.2 (the person did not see the answer), the rest are accepted or corrected.
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut unit = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let records: Vec<Record> = (0..4_000_i64)
        .map(|i| {
            let score = unit();
            let right = unit() < score;
            let (shown, settled) = if right { ("a", "a") } else { ("a", "b") };
            if i % 5 == 0 {
                let mut r = line(
                    &format!("r{i:05}"),
                    i,
                    Source::External,
                    Some(score),
                    shown,
                    SettlementKind::Check,
                    settled,
                );
                r.probabilities.insert("team".to_owned(), 0.2);
                r
            } else {
                let kind = if right {
                    SettlementKind::Accept
                } else {
                    SettlementKind::Correct
                };
                line(
                    &format!("r{i:05}"),
                    i,
                    Source::External,
                    Some(score),
                    shown,
                    kind,
                    settled,
                )
            }
        })
        .collect();

    let decisions = scored_decisions(&records, "team");
    assert_eq!(decisions.len(), 4_000);
    assert_eq!(decisions.iter().filter(|d| d.weight == 5.0).count(), 800);

    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 30,
        counting: None,
    };
    let chosen = select_score_threshold(&decisions, choice)
        .chosen
        .expect("a calibrated judge reaches 0.8");
    assert!(
        (0.7..=0.9).contains(&chosen.threshold),
        "threshold {}",
        chosen.threshold
    );
    assert!(chosen.precision >= 0.8, "precision {}", chosen.precision);
}
