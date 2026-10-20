//! Usage counts: each settlement kind lands where its meaning puts it, and the realized precision rests only on the
//! settlements that say whether the answer was right.

use gil::schema::SettlementKind::{self, Accept, Check, Correct, Enter, Fill, Reject};
use gil::usage::{Decision, usage};

fn d<'a>(
    answer: Option<&'a str>,
    settled: Option<&'a str>,
    settlement: Option<SettlementKind>,
    probability: f64,
) -> Decision<'a> {
    Decision {
        attribute: "team",
        answer,
        settled,
        settlement,
        probability,
        promise: answer.map(|_| 0.85),
    }
}

#[test]
fn every_settlement_kind_lands_where_its_meaning_puts_it() {
    let counts = usage([
        d(Some("network"), Some("network"), Some(Accept), 1.0),
        d(Some("network"), Some("network"), Some(Accept), 1.0),
        d(Some("network"), Some("security"), Some(Correct), 1.0),
        d(Some("network"), None, Some(Reject), 1.0),
        d(Some("network"), Some("network"), Some(Fill), 1.0),
        d(Some("network"), Some("network"), Some(Check), 0.25),
        d(Some("network"), Some("security"), Some(Check), 0.5),
        d(None, Some("facilities"), Some(Enter), 1.0),
        d(None, None, None, 1.0),
        d(Some("network"), Some("network"), Some(Enter), 1.0),
    ]);
    let team = &counts["team"];
    assert_eq!(
        (team.decisions, team.answered, team.abstained()),
        (10, 8, 2)
    );
    assert_eq!(
        (team.accepted, team.corrected, team.rejected, team.filled),
        (2, 1, 1, 1)
    );
    assert_eq!((team.checked, team.checked_right, team.entered), (2, 1, 1));
    assert_eq!(team.entered_over_answer, 1);
    // Right: 2 accepts + a check worth 4; judged: those + 1 correction + a check worth 2.
    assert_eq!(team.realized(), Some(6.0 / 9.0));
    assert!((team.promise().unwrap() - 0.85).abs() < 1e-12);
    assert_eq!(team.answer_rate(), Some(0.8));
}

#[test]
fn nothing_judged_realizes_nothing() {
    let counts = usage([d(Some("network"), Some("network"), Some(Fill), 1.0)]);
    assert_eq!(counts["team"].realized(), None);
    assert!(usage(std::iter::empty()).is_empty());
}
