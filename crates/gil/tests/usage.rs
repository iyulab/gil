//! Usage counts: each settlement kind lands where its meaning puts it, and the realized precision rests only on the
//! settlements that say whether the answer was right.

use gil::decider::Source;
use gil::schema::Record;
use gil::schema::SettlementKind::{
    self, Accept, AcceptOther, Check, Correct, CorrectOther, Enter, Fill, Reject,
};
use gil::usage::usage;

/// A record of its own decision settling `team` as `settled` by `settlement`, with this decider's `answer` in view when
/// there is one.
fn d(
    answer: Option<&str>,
    settled: Option<&str>,
    settlement: Option<SettlementKind>,
    probability: f64,
) -> Record {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let mut record = Record {
        id: format!(
            "r{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ),
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
        // Turned down and kept as such: the answer in view, the settlement a `Reject`.
        d(Some("network"), None, Some(Reject), 1.0),
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
    assert_eq!(
        (team.decisions, team.answered, team.abstained()),
        (10, 9, 1)
    );
    assert_eq!(
        (team.accepted, team.corrected, team.rejected, team.filled),
        (2, 1, 2, 1)
    );
    assert_eq!((team.checked, team.checked_right, team.entered), (2, 1, 1));
    assert_eq!(team.entered_over_answer, 1);
    // Right: 2 accepts + a check worth 4; judged: those + 1 correction + a check worth 2.
    assert_eq!(team.realized(), Some(6.0 / 9.0));
    assert!((team.promise().unwrap() - 0.85).abs() < 1e-12);
    assert_eq!(team.answer_rate(), Some(9.0 / 10.0));
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

#[test]
fn a_check_with_a_probability_no_draw_has_is_not_judged() {
    // A record built in code can hold what a read line refuses; such a check counts as nothing rather than as
    // infinitely many decisions (0), fewer than none (negative) or less than one (above 1).
    for probability in [0.0, -0.5, 4.0] {
        let record = d(Some("network"), Some("network"), Some(Check), probability);
        let counts = usage([&record]);
        assert_eq!(counts["team"].checked, 0, "{probability}");
        assert_eq!(counts["team"].realized(), None, "{probability}");
    }
}

/// `record` as the decision `id`, settled at `at`.
fn at(mut record: Record, id: &str, at: i64) -> Record {
    record.id = id.to_owned();
    record.settled_at = at;
    record
}

#[test]
fn a_first_check_is_told_from_a_later_audit_by_when_the_id_was_first_settled() {
    let mut audit = at(d(None, Some("network"), Some(Check), 0.05), "a", 20);
    audit.sources.clear();
    let mut unanswered = at(
        d(Some("network"), Some("network"), Some(Check), 0.1),
        "c",
        5,
    );
    unanswered.suggested.clear();
    let records = [
        // The audit is given before the settlement it audits: the order is `settled_at`'s, not the input's.
        audit,
        at(
            d(Some("network"), Some("network"), Some(Accept), 1.0),
            "a",
            10,
        ),
        at(
            d(Some("network"), Some("network"), Some(Check), 0.1),
            "b",
            10,
        ),
        at(
            d(Some("network"), Some("security"), Some(Check), 0.1),
            "d",
            10,
        ),
        unanswered,
    ];
    let team = &usage(&records)["team"];
    assert_eq!(team.audits, 1);
    assert_eq!((team.first_checks, team.first_checks_right), (2, 1));
    assert_eq!(team.first_checks_without_answer, 1);
    assert_eq!(team.first_checks_realized(), Some(0.5));
    assert_eq!((team.checked, team.checked_right), (2, 1));
    // The audit is not another decision: four decisions, the one without an answer kept among them.
    assert_eq!(team.decisions, 4);
}

#[test]
fn an_audit_that_keeps_the_answer_is_not_another_decision_but_judges_it() {
    let records = [
        at(
            d(Some("network"), Some("network"), Some(Accept), 1.0),
            "a",
            10,
        ),
        at(
            d(Some("network"), Some("security"), Some(Check), 0.05),
            "a",
            20,
        ),
    ];
    let team = &usage(&records)["team"];
    // One decision, audited once; the audit judges the answer like any check: wrong, weighing 1 ÷ 0.05.
    assert_eq!((team.decisions, team.answered, team.audits), (1, 1, 1));
    assert_eq!(
        (team.checked, team.checked_right, team.first_checks),
        (1, 0, 0)
    );
    assert_eq!(team.realized(), Some(1.0 / 21.0));
}

#[test]
fn of_two_settlements_at_the_same_moment_the_earlier_given_is_first() {
    let records = [
        at(
            d(Some("network"), Some("network"), Some(Check), 0.1),
            "a",
            10,
        ),
        at(
            d(Some("network"), Some("network"), Some(Check), 0.1),
            "a",
            10,
        ),
    ];
    let team = &usage(&records)["team"];
    assert_eq!((team.first_checks, team.audits, team.decisions), (1, 1, 1));
}

#[test]
fn answers_shown_under_a_promise_display_draw_are_counted_by_the_draw() {
    use gil::schema::PromiseShown;
    use gil::usage::PromiseDisplay;
    let shown = |record: Record, displayed: bool, probability: f64| {
        let mut record = record;
        record.promise_shown.insert(
            "team".to_owned(),
            PromiseShown::new(displayed, probability).unwrap(),
        );
        record
    };
    let records = [
        shown(
            d(Some("network"), Some("network"), Some(Accept), 1.0),
            true,
            0.5,
        ),
        shown(
            d(Some("network"), Some("network"), Some(Accept), 1.0),
            false,
            0.5,
        ),
        shown(
            d(Some("network"), Some("security"), Some(Correct), 1.0),
            false,
            0.5,
        ),
        shown(
            d(Some("network"), Some("network"), Some(Accept), 1.0),
            true,
            1.0,
        ),
        shown(
            d(Some("network"), Some("security"), Some(Correct), 1.0),
            false,
            0.0,
        ),
        // Not accepted or corrected: no part in the display counts.
        shown(
            d(Some("network"), Some("network"), Some(Fill), 1.0),
            true,
            0.5,
        ),
        // Accepted with no draw kept: counted as before, in no display entry.
        d(Some("network"), Some("network"), Some(Accept), 1.0),
    ];
    let team = &usage(&records)["team"];
    assert_eq!(team.accepted, 4);
    assert_eq!(
        team.promise_display,
        vec![
            PromiseDisplay {
                probability: 0.0,
                hidden_corrected: 1,
                ..PromiseDisplay::default()
            },
            PromiseDisplay {
                probability: 0.5,
                shown_accepted: 1,
                hidden_accepted: 1,
                hidden_corrected: 1,
                ..PromiseDisplay::default()
            },
            PromiseDisplay {
                probability: 1.0,
                shown_accepted: 1,
                ..PromiseDisplay::default()
            },
        ]
    );
}
