//! Usage counts: each settlement kind lands where its meaning puts it, and the realized precision rests only on the
//! settlements that say whether the answer was right.

use gil::decider::Source;
use gil::schema::Record;
use gil::schema::SettlementKind::{
    self, Accept, AcceptOther, Check, Correct, CorrectOther, Enter, Fill, Reject,
};
use gil::usage::usage;

/// A record settling `team` as `settled` by `settlement`, with this decider's `answer` in view when there is one.
fn d(
    answer: Option<&str>,
    settled: Option<&str>,
    settlement: Option<SettlementKind>,
    probability: f64,
) -> Record {
    let mut record = Record {
        id: "r".to_owned(),
        ..Record::default()
    };
    if let Some(value) = settled {
        record.values.insert("team".to_owned(), value.to_owned());
    }
    if let Some(kind) = settlement {
        record.settlements.insert("team".to_owned(), kind);
        if kind == Check {
            record.probabilities.insert("team".to_owned(), probability);
        }
    }
    if let Some(value) = answer {
        record.suggested.insert("team".to_owned(), value.to_owned());
        record.sources.insert("team".to_owned(), Source::KeyMemory);
        record.promises.insert("team".to_owned(), 0.85);
    }
    record
}

/// A record settling `team` as `settled` over a suggestion `shown` that was not this decider's.
fn other(shown: &str, settled: &str, settlement: SettlementKind) -> Record {
    let mut record = d(None, Some(settled), Some(settlement), 1.0);
    record.suggested.insert("team".to_owned(), shown.to_owned());
    record
}

#[test]
fn every_settlement_kind_lands_where_its_meaning_puts_it() {
    let records = [
        d(Some("network"), Some("network"), Some(Accept), 1.0),
        d(Some("network"), Some("network"), Some(Accept), 1.0),
        d(Some("network"), Some("security"), Some(Correct), 1.0),
        // Turned down: the answer in view, no value and no settlement.
        d(Some("network"), None, None, 1.0),
        d(Some("network"), Some("network"), Some(Fill), 1.0),
        d(Some("network"), Some("network"), Some(Check), 0.25),
        d(Some("network"), Some("security"), Some(Check), 0.5),
        d(None, Some("facilities"), Some(Enter), 1.0),
        d(Some("network"), Some("network"), Some(Enter), 1.0),
        // Neither settled nor answered: not a decision the record speaks of.
        d(None, None, None, 1.0),
    ];
    let counts = usage(&records);
    let team = &counts["team"];
    assert_eq!((team.decisions, team.answered, team.abstained()), (9, 8, 1));
    assert_eq!(
        (team.accepted, team.corrected, team.rejected, team.filled),
        (2, 1, 1, 1)
    );
    assert_eq!((team.checked, team.checked_right, team.entered), (2, 1, 1));
    assert_eq!(team.entered_over_answer, 1);
    // Right: 2 accepts + a check worth 4; judged: those + 1 correction + a check worth 2.
    assert_eq!(team.realized(), Some(6.0 / 9.0));
    assert!((team.promise().unwrap() - 0.85).abs() < 1e-12);
    assert_eq!(team.answer_rate(), Some(8.0 / 9.0));
}

#[test]
fn an_accepted_answer_counts_without_its_layer_kept() {
    let mut record = d(Some("network"), Some("network"), Some(Accept), 1.0);
    record.sources.clear();
    record.promises.clear();
    let counts = usage([&record]);
    assert_eq!((counts["team"].answered, counts["team"].promised), (1, 0));
    assert_eq!(counts["team"].realized(), Some(1.0));
}

#[test]
fn a_check_with_no_suggestion_kept_is_not_judged() {
    let mut record = d(Some("network"), Some("network"), Some(Check), 0.5);
    record.suggested.clear();
    let counts = usage([&record]);
    assert_eq!((counts["team"].answered, counts["team"].checked), (1, 0));
    assert_eq!(counts["team"].realized(), None);
}

#[test]
fn nothing_judged_realizes_nothing() {
    let counts = usage([&d(Some("network"), Some("network"), Some(Fill), 1.0)]);
    assert_eq!(counts["team"].realized(), None);
    assert!(usage(std::iter::empty()).is_empty());
}

#[test]
fn a_value_settled_over_another_suggestion_is_counted_on_its_own_and_grades_no_answer() {
    let mut over_answer = other("security", "security", AcceptOther);
    over_answer
        .sources
        .insert("team".to_owned(), Source::KeyMemory);
    let records = [
        d(Some("network"), Some("network"), Some(Accept), 1.0),
        // The host showed its own suggestion: in place of an abstention, and once in place of an answer.
        other("facilities", "facilities", AcceptOther),
        other("facilities", "security", CorrectOther),
        over_answer,
    ];
    let counts = usage(&records);
    let team = &counts["team"];
    assert_eq!((team.accepted_other, team.corrected_other), (2, 1));
    assert_eq!((team.decisions, team.answered), (4, 2));
    assert_eq!((team.accepted, team.corrected, team.entered), (1, 0, 0));
    assert_eq!(team.entered_over_answer, 0);
    assert_eq!(team.realized(), Some(1.0));
}

#[test]
fn every_kind_reads_back_from_its_name() {
    for kind in [
        Accept,
        Correct,
        Reject,
        SettlementKind::Revert,
        SettlementKind::Restore,
        SettlementKind::Set,
        Enter,
        Fill,
        Check,
        AcceptOther,
        CorrectOther,
    ] {
        assert_eq!(SettlementKind::parse(&format!("{kind:?}")), Some(kind));
    }
    assert_eq!(SettlementKind::parse("Other"), None);
}
