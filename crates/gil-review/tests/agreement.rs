//! Agreement. The cases are those of the reference implementation's tests (three outcomes: same, different, cannot
//! tell), compared to the digits they compare.

use gil_review::{ReviewError, Verdict, agreement};

const SAME: u8 = 0;
const DIFFERENT: u8 = 1;
const CANNOT_TELL: u8 = 2;

fn close(expected: f64, actual: f64) -> bool {
    let scale = 1e12;
    (expected * scale).round_ties_even() == (actual * scale).round_ties_even()
}

struct Pairs(Vec<(String, u8, u8)>);

impl Pairs {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn add(mut self, times: usize, first: u8, second: u8) -> Self {
        for _ in 0..times {
            let i = self.0.len();
            self.0.push((format!("r{i}"), first, second));
        }
        self
    }
    fn verdicts(&self) -> Vec<Verdict<'_>> {
        self.0
            .iter()
            .flat_map(|(item, a, b)| {
                [
                    Verdict {
                        item,
                        candidate: "c",
                        outcome: *a,
                        reviewer: "alice",
                    },
                    Verdict {
                        item,
                        candidate: "c",
                        outcome: *b,
                        reviewer: "bob",
                    },
                ]
            })
            .collect()
    }
}

#[test]
fn perfect_agreement_scores_one_on_every_coefficient() {
    let pairs = Pairs::new().add(3, SAME, SAME).add(3, DIFFERENT, DIFFERENT);
    let a = agreement(&pairs.verdicts(), 3).unwrap();
    assert_eq!(a.compared_pairs, 6);
    assert!(close(1.0, a.observed) && close(1.0, a.kappa) && close(1.0, a.ac1));
    assert!(close(1.0, a.specific[0]) && close(1.0, a.specific[1]));
    assert!(a.specific[usize::from(CANNOT_TELL)].is_nan());
    assert!(close(0.5, a.prevalence_max));
}

#[test]
fn kappa_and_ac1_match_a_hand_calculation() {
    let pairs = Pairs::new()
        .add(5, SAME, SAME)
        .add(3, DIFFERENT, DIFFERENT)
        .add(2, SAME, DIFFERENT);
    let a = agreement(&pairs.verdicts(), 3).unwrap();
    assert!(close(0.8, a.observed));
    assert!(close(0.6, a.kappa));
    assert!(close((0.8 - 0.24) / 0.76, a.ac1));
    assert!(close(10.0 / 12.0, a.specific[0]));
    assert!(close(6.0 / 8.0, a.specific[1]));
    assert!(close(12.0 / 20.0, a.prevalence_max));
}

#[test]
fn under_one_sided_prevalence_kappa_collapses_and_ac1_holds() {
    let pairs = Pairs::new()
        .add(48, SAME, SAME)
        .add(1, SAME, DIFFERENT)
        .add(1, DIFFERENT, SAME);
    let a = agreement(&pairs.verdicts(), 3).unwrap();
    assert!(close(0.96, a.observed));
    assert!(a.kappa < 0.0, "{}", a.kappa);
    assert!(a.ac1 > 0.9, "{}", a.ac1);
    assert!(close(0.98, a.prevalence_max));
}

#[test]
fn kappa_is_undefined_when_both_used_one_outcome_and_ac1_is_not() {
    let a = agreement(&Pairs::new().add(4, SAME, SAME).verdicts(), 3).unwrap();
    assert!(a.kappa.is_nan());
    assert!(close(1.0, a.ac1));
}

#[test]
fn single_reviewed_pairs_are_passed_over_and_a_third_verdict_is_refused() {
    let v = |item, reviewer| Verdict {
        item,
        candidate: "c",
        outcome: SAME,
        reviewer,
    };
    let mut verdicts = vec![v("r0", "alice"), v("r1", "alice"), v("r1", "bob")];
    assert_eq!(agreement(&verdicts, 3).unwrap().compared_pairs, 1);
    verdicts.push(v("r1", "carol"));
    assert!(matches!(
        agreement(&verdicts, 3),
        Err(ReviewError::MoreThanTwoVerdicts { .. })
    ));
    assert!(matches!(
        agreement(&[v("r2", "alice"), v("r2", "alice")], 3),
        Err(ReviewError::SameReviewerTwice { .. })
    ));
}

#[test]
fn nothing_double_reviewed_is_refused_rather_than_scored_as_agreement() {
    let one = [Verdict {
        item: "r0",
        candidate: "c",
        outcome: SAME,
        reviewer: "alice",
    }];
    assert_eq!(agreement(&one, 3), Err(ReviewError::NothingCompared));
    assert_eq!(agreement(&one, 1), Err(ReviewError::Categories));
    let bad = [Verdict {
        outcome: 3,
        ..one[0]
    }];
    assert!(matches!(
        agreement(&bad, 3),
        Err(ReviewError::Outcome { .. })
    ));
}

#[test]
fn the_reviewer_order_is_by_id_not_by_input() {
    let pairs = Pairs::new().add(5, SAME, SAME).add(2, SAME, DIFFERENT);
    let mut verdicts = pairs.verdicts();
    let forward = agreement(&verdicts, 2).unwrap();
    verdicts.reverse();
    assert_eq!(agreement(&verdicts, 2).unwrap(), forward);
}
