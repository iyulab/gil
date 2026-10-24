//! The least-cost rule: each layer's best value gets a calibrated chance of being right, the more likely layer answers,
//! and one chance threshold is chosen where answering costs least when a wrong answer costs 1 / (1 − target) entries.
//! The answers it chooses are right at least the target's share of the time on the lookups it was chosen on.

use std::collections::BTreeMap;

use gil::decider::{Decider, Source};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Calibration, Choice, LayerOptions, LayerThresholds, ThresholdRule, select_layers,
};
use gil::schema::{Attribute, AttributeRole, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;
const TEAMS: [&str; 4] = ["network", "desk", "facilities", "billing"];

fn schema() -> Schema {
    Schema::new(
        "ticket",
        vec![
            Attribute::new("source", AttributeRole::Observed),
            Attribute::new("summary", AttributeRole::Observed),
            Attribute::new("team", AttributeRole::Decided),
        ],
    )
    .expect("a valid schema")
}

/// A small linear congruential generator, so the records come out the same on every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// `n` records from twelve sources, each settled to its source's usual team — more often for some sources than for
/// others — and saying a word of their team in their summary now and then, so both layers have something to go on and
/// neither is always right.
fn records(n: usize) -> Vec<Record> {
    let mut random = Lcg(23);
    (0..n)
        .map(|i| {
            let source = random.below(12) as usize;
            let usual = TEAMS[source % TEAMS.len()];
            // Sources 0–3 keep to their team nineteen times in twenty; the rest three times in five.
            let loyal = if source < 4 { 19 } else { 12 };
            let team = if random.below(20) < loyal {
                usual
            } else {
                TEAMS[random.below(TEAMS.len() as u64) as usize]
            };
            let word = if random.below(3) == 0 {
                format!("{team}-word")
            } else {
                format!("w{}", random.below(400))
            };
            Record {
                id: format!("t{i}"),
                values: [
                    ("source".to_owned(), format!("s{source}")),
                    (
                        "summary".to_owned(),
                        format!("{word} w{}", random.below(2_000)),
                    ),
                    ("team".to_owned(), team.to_owned()),
                ]
                .into_iter()
                .collect(),
                settled_at: i as i64 * MINUTE,
                ..Record::default()
            }
        })
        .collect()
}

fn chosen(
    records: &[Record],
    target: f64,
    rule: ThresholdRule,
    sink: Option<&mut dyn FnMut(&DecisionRecord)>,
) -> LayerThresholds {
    select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        records,
        Choice {
            target_precision: target,
            minimum_answered: 10,
            counting: None,
        },
        LayerOptions {
            rule,
            sink,
            ..LayerOptions::default()
        },
    )
}

/// Each lookup's answer by the calibration, counted again from the replay's decision records: the more likely layer's
/// chance and whether it matched, the key layer on a tie.
fn candidates(decisions: &[DecisionRecord], calibration: &Calibration) -> Vec<(f64, bool)> {
    decisions
        .iter()
        .filter_map(|d| {
            let caught = d.key.as_ref().is_some_and(|k| k.caught);
            let k = d.key.as_ref().filter(|k| !k.caught).and_then(|k| {
                Calibration::chance(&calibration.key, k.strength)
                    .map(|c| (c, k.correct == Some(true)))
            });
            let m = d.similar.as_ref().filter(|_| !caught).and_then(|s| {
                Calibration::chance(&calibration.memory, s.score)
                    .map(|c| (c, s.correct == Some(true)))
            });
            match (k, m) {
                (Some(k), Some(m)) if m.0 > k.0 => Some(m),
                (Some(k), _) => Some(k),
                (None, m) => m,
            }
        })
        .collect()
}

fn gain(candidates: &[(f64, bool)], threshold: f64, rho: f64) -> f64 {
    candidates
        .iter()
        .filter(|c| c.0 >= threshold)
        .map(|c| if c.1 { 1.0 } else { 1.0 - rho })
        .sum()
}

#[test]
fn the_answers_chosen_hold_the_target_and_cost_least() {
    let records = records(3_000);
    for target in [0.7, 0.8, 0.9] {
        let mut decisions = Vec::new();
        let layers = chosen(
            &records,
            target,
            ThresholdRule::LeastCost,
            Some(&mut |d| decisions.push(d.clone())),
        );
        let calibration = layers.calibration.clone().expect("the rule's calibration");
        let threshold = calibration.threshold.expect("a threshold that pays");
        let all = candidates(&decisions, &calibration);
        let given: Vec<&(f64, bool)> = all.iter().filter(|c| c.0 >= threshold).collect();

        // Right at least the target's share of the time, on average.
        let right = given.iter().filter(|c| c.1).count() as f64;
        assert!(
            right / given.len() as f64 >= target,
            "{target}: {right} of {}",
            given.len()
        );

        // No other chance seen gains more.
        let rho = 1.0 / (1.0 - target);
        let best = gain(&all, threshold, rho);
        for other in all.iter().map(|c| c.0) {
            assert!(
                gain(&all, other, rho) <= best + 1e-9,
                "{target}: {other} beats {threshold}"
            );
        }

        // The layers' shares add up to the answers given, and each layer answers from where its chance reaches the
        // threshold.
        let answered = layers.key.chosen.as_ref().map_or(0, |c| c.answered)
            + layers.memory.chosen.as_ref().map_or(0, |c| c.answered);
        assert_eq!(answered, given.len(), "{target}");
        if let Some(key) = &layers.key.chosen {
            assert!(Calibration::chance(&calibration.key, key.threshold).unwrap() >= threshold);
            assert_eq!(key.similarity_floor, None);
        }
    }
}

#[test]
fn a_decider_answers_with_the_layer_the_rule_picks() {
    let records = records(2_600);
    let layers = chosen(&records[..2_000], 0.8, ThresholdRule::LeastCost, None);
    let calibration = layers.calibration.clone().expect("the rule's calibration");
    let threshold = calibration.threshold.expect("a threshold that pays");
    let schema = schema().with_layers("team", &layers).unwrap();
    assert_eq!(
        schema.attribute("team").calibration.as_ref(),
        Some(&calibration)
    );
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(&schema, &records[..2_000]);

    let (mut by_layer, mut overtaken) = (BTreeMap::new(), 0);
    for record in &records[2_000..] {
        let mut values = record.values.clone();
        values.remove("team");
        let mut seen = Vec::new();
        decider.answer_with(&schema, &record.id, &values, &BTreeMap::new(), &mut |d| {
            seen.push(d.clone())
        });
        let d = seen
            .into_iter()
            .find(|d| d.attribute == "team")
            .expect("team asked");
        // What the rule makes of the same lookup.
        let k = d
            .key
            .as_ref()
            .filter(|k| !k.caught)
            .and_then(|k| Calibration::chance(&calibration.key, k.strength));
        let m = d
            .similar
            .as_ref()
            .filter(|_| !d.key.as_ref().is_some_and(|k| k.caught))
            .and_then(|s| Calibration::chance(&calibration.memory, s.score));
        let expected = match (k, m) {
            (Some(k), Some(m)) if m > k => (m >= threshold).then_some(Source::SimilarRecord),
            (Some(k), _) => (k >= threshold).then_some(Source::KeyMemory),
            (None, Some(m)) => (m >= threshold).then_some(Source::SimilarRecord),
            (None, None) => None,
        };
        assert_eq!(
            d.answered.as_ref().map(|a| a.layer),
            expected,
            "{}",
            record.id
        );
        if let Some(layer) = expected {
            *by_layer.entry(format!("{layer:?}")).or_insert(0) += 1;
        }
        // The similar record layer answered although the key layer reached its threshold too.
        if expected == Some(Source::SimilarRecord) && k.is_some_and(|k| k >= threshold) {
            overtaken += 1;
        }
    }
    assert!(by_layer.len() == 2, "both layers answer: {by_layer:?}");
    assert!(overtaken > 0, "the more likely layer goes first");
}

#[test]
fn a_calibration_out_of_shape_is_refused() {
    let mut team = Attribute::new("team", AttributeRole::Decided);
    team.calibration = Some(Calibration {
        key: vec![
            gil::replay::CalibrationStep {
                score: 2.0,
                chance: 0.5,
            },
            gil::replay::CalibrationStep {
                score: 1.0,
                chance: 0.9,
            },
        ],
        memory: Vec::new(),
        threshold: Some(0.8),
    });
    let refused = Schema::new(
        "ticket",
        vec![Attribute::new("source", AttributeRole::Observed), team],
    );
    assert!(refused.unwrap_err().contains("calibration"));
}

#[test]
fn the_calibration_rises_with_the_score() {
    let calibration = chosen(&records(2_000), 0.8, ThresholdRule::LeastCost, None)
        .calibration
        .expect("the rule's calibration");
    for steps in [&calibration.key, &calibration.memory] {
        assert!(!steps.is_empty());
        assert!(
            steps
                .windows(2)
                .all(|w| w[0].score < w[1].score && w[0].chance < w[1].chance)
        );
        assert!(steps.iter().all(|s| (0.0..=1.0).contains(&s.chance)));
    }
    assert_eq!(Calibration::chance(&[], 0.5), None);
    // Below the first step, the first step's chance holds.
    let first = calibration.key[0];
    assert_eq!(
        Calibration::chance(&calibration.key, first.score - 1.0),
        Some(first.chance)
    );
}

#[test]
fn the_other_rules_carry_no_calibration() {
    let records = records(800);
    for rule in [ThresholdRule::EveryBand, ThresholdRule::Pooled] {
        assert_eq!(chosen(&records, 0.8, rule, None).calibration, None);
    }
}

#[test]
fn a_target_of_one_answers_only_where_nothing_was_wrong() {
    let mut decisions = Vec::new();
    let layers = chosen(
        &records(1_500),
        1.0,
        ThresholdRule::LeastCost,
        Some(&mut |d| decisions.push(d.clone())),
    );
    let calibration = layers.calibration.expect("the rule's calibration");
    if let Some(threshold) = calibration.threshold {
        assert!(
            candidates(&decisions, &calibration)
                .iter()
                .filter(|c| c.0 >= threshold)
                .all(|c| c.1)
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn the_calibration_round_trips_and_names_the_rule() {
    let layers = chosen(&records(800), 0.8, ThresholdRule::LeastCost, None);
    let text = serde_json::to_string(&layers).unwrap();
    assert_eq!(
        serde_json::from_str::<LayerThresholds>(&text).unwrap(),
        layers
    );
    assert_eq!(
        serde_json::to_string(&ThresholdRule::LeastCost).unwrap(),
        "\"least_cost\""
    );
}
