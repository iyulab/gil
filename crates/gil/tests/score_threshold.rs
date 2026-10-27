//! A threshold for a judge outside Gil, chosen from its scores and what people settled (experimental) — the same
//! threshold, answer rate and promise a memory layer gets, from nothing but `(score, right)` in settlement order.

use gil::replay::{Choice, ScoredDecision, select_score_threshold};

/// A small deterministic generator (SplitMix64), so the decisions are the same on every run.
struct Draws(u64);

impl Draws {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }
}

fn choice(target: f64) -> Choice<'static> {
    Choice {
        target_precision: target,
        minimum_answered: 30,
        counting: None,
    }
}

/// A judge whose score is the chance it is right (calibrated): `n` decisions settled one tick apart.
fn calibrated(n: usize, seed: u64) -> Vec<ScoredDecision> {
    let mut draws = Draws(seed);
    (0..n)
        .map(|i| {
            let score = draws.unit();
            let right = draws.unit() < score;
            ScoredDecision::new(format!("case-{i:05}"), i as i64, Some(score), right)
        })
        .collect()
}

#[test]
fn a_calibrated_judge_gets_a_threshold_that_keeps_the_target_with_a_promise_beneath_it() {
    let replay = select_score_threshold(&calibrated(6_000, 7), choice(0.8));
    let chosen = replay.chosen.expect("a calibrated judge reaches 0.8");
    assert!(
        (0.7..=0.9).contains(&chosen.threshold),
        "the threshold sits near the target for a calibrated score, got {}",
        chosen.threshold
    );
    assert!(chosen.precision >= 0.8, "precision {}", chosen.precision);
    assert!(chosen.promise <= chosen.precision);
    assert!(chosen.answer_rate > 0.0 && chosen.answer_rate < 1.0);
    assert_eq!(chosen.lookups, 6_000);
}

#[test]
fn the_answers_keep_the_target_on_decisions_the_threshold_was_not_chosen_on() {
    // The promise is the bottom of a Wilson interval at one standard error — about one chance in six that the precision
    // is below it — so a fresh run may fall a hair under it; what must hold is the target, and the promise must be close.
    let chosen = select_score_threshold(&calibrated(6_000, 11), choice(0.8))
        .chosen
        .expect("a threshold stands");
    let fresh = calibrated(20_000, 12);
    let (answered, right) = fresh
        .iter()
        .filter(|d| d.score.is_some_and(|s| s >= chosen.threshold))
        .fold((0, 0), |(a, r), d| (a + 1, r + usize::from(d.right)));
    assert!(answered > 1_000);
    let realized = right as f64 / answered as f64;
    assert!(realized >= 0.8, "realized {realized} under the target");
    assert!(
        (realized - chosen.promise).abs() < 0.03,
        "realized {realized} against promise {}",
        chosen.promise
    );
}

#[test]
fn the_order_decisions_are_passed_in_does_not_matter() {
    let mut decisions = calibrated(2_000, 3);
    let forward = select_score_threshold(&decisions, choice(0.85));
    decisions.reverse();
    assert_eq!(forward, select_score_threshold(&decisions, choice(0.85)));
}

#[test]
fn a_decision_the_judge_gave_no_candidate_for_is_asked_and_not_answered() {
    let mut decisions = calibrated(3_000, 5);
    let answered_only = select_score_threshold(&decisions, choice(0.8))
        .chosen
        .unwrap();
    for i in 0..1_000 {
        decisions.push(ScoredDecision::new(
            format!("silent-{i:05}"),
            10_000 + i,
            None,
            false,
        ));
    }
    let with_silence = select_score_threshold(&decisions, choice(0.8))
        .chosen
        .unwrap();
    assert_eq!(with_silence.threshold, answered_only.threshold);
    assert_eq!(with_silence.lookups, 4_000);
    assert!(with_silence.answer_rate < answered_only.answer_rate);
}

#[test]
fn a_score_that_says_nothing_gets_no_threshold() {
    let mut draws = Draws(21);
    let decisions: Vec<ScoredDecision> = (0..4_000)
        .map(|i| {
            let score = draws.unit();
            let right = draws.unit() < 0.5; // right half the time whatever the score
            ScoredDecision::new(format!("case-{i:05}"), i, Some(score), right)
        })
        .collect();
    assert!(
        select_score_threshold(&decisions, choice(0.9))
            .chosen
            .is_none()
    );
}

#[test]
fn a_checked_decision_counts_at_its_weight() {
    let mut decisions = calibrated(3_000, 9);
    let plain = select_score_threshold(&decisions, choice(0.8))
        .chosen
        .unwrap();
    for decision in decisions.iter_mut() {
        decision.weight = 4.0;
    }
    let heavy = select_score_threshold(&decisions, choice(0.8))
        .chosen
        .unwrap();
    assert_eq!(heavy.threshold, plain.threshold);
    assert_eq!(heavy.answered, plain.answered);
    assert!((heavy.answered_weight - 4.0 * plain.answered_weight).abs() < 1e-6);
    assert!((heavy.answer_rate - plain.answer_rate).abs() < 1e-12);
}

#[test]
#[should_panic(expected = "a score must be finite")]
fn a_score_that_is_not_a_number_is_refused() {
    let decisions = vec![ScoredDecision::new("a", 0, Some(f64::NAN), true)];
    let _ = select_score_threshold(&decisions, choice(0.8));
}

#[test]
#[should_panic(expected = "a weight must be positive and finite")]
fn a_weight_of_zero_is_refused() {
    let decisions = vec![ScoredDecision::new("a", 0, Some(0.5), true).weighted(0.0)];
    let _ = select_score_threshold(&decisions, choice(0.8));
}
