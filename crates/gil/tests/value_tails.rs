//! Value by settled value, how the chosen thresholds answered: a value the layers keep taking for another shows a
//! share far above the attribute's average, the value given in its place is named, and asking for the report changes
//! nothing else the replay chooses.

use std::collections::BTreeMap;

use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, LayerOptions, LayerThresholds, VALUE_TAIL_DECISIONS, ValueTail, select_layers,
};
use gil::schema::{Attribute, AttributeRole, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;

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

/// `n` records from four sources, each settled to its source's team — except that a third of the records from the
/// network source are security's, saying nothing in their own words that tells them apart. The key layer answers them
/// as network, so security is the value taken for another.
fn records(n: usize) -> Vec<Record> {
    let mut random = Lcg(11);
    (0..n)
        .map(|i| {
            let source = random.below(4) as usize;
            let usual = ["network", "desk", "facilities", "billing"][source];
            let team = if source == 0 && random.below(3) == 0 {
                "security"
            } else {
                usual
            };
            Record {
                id: format!("t{i}"),
                values: [
                    ("source".to_owned(), format!("s{source}")),
                    (
                        "summary".to_owned(),
                        format!("w{} w{}", random.below(3_000), random.below(3_000)),
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
    value_tails: bool,
    sink: Option<&mut dyn FnMut(&DecisionRecord)>,
) -> LayerThresholds {
    select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema(),
        "team",
        records,
        Choice {
            target_precision: 0.6,
            minimum_answered: 10,
            counting: None,
        },
        LayerOptions {
            sink,
            value_tails,
            ..LayerOptions::default()
        },
    )
}

/// The report, counted again from the replay's decision records as a decider answers with the chosen thresholds.
fn by_hand(decisions: &[DecisionRecord], chosen: &LayerThresholds) -> Vec<ValueTail> {
    let key = chosen.key.chosen.as_ref().map(|c| c.threshold);
    let memory = chosen.memory.chosen.as_ref();
    let mut tally: BTreeMap<String, (usize, usize, BTreeMap<String, usize>)> = BTreeMap::new();
    // The first record is asked about by neither layer: nothing was in memory yet.
    for d in decisions.iter().filter(|d| d.order != Some(1)) {
        let settled = d.settled.clone().expect("a settled value");
        let caught = d.key.as_ref().is_some_and(|k| k.caught);
        let answer = d
            .key
            .as_ref()
            .filter(|k| !k.caught && key.is_some_and(|t| k.strength >= t))
            .map(|k| k.value.clone())
            .or_else(|| {
                let s = d.similar.as_ref().filter(|_| !caught)?;
                let m = memory?;
                (s.score >= m.threshold
                    && !(s.voted && m.similarity_floor.is_some_and(|f| s.nearest < f)))
                .then(|| s.value.clone())
            });
        let entry = tally.entry(settled.clone()).or_default();
        entry.0 += 1;
        if let Some(a) = answer.filter(|a| *a != settled) {
            entry.1 += 1;
            *entry.2.entry(a).or_default() += 1;
        }
    }
    tally
        .into_iter()
        .filter(|(_, (n, _, _))| *n >= VALUE_TAIL_DECISIONS)
        .map(|(value, (n, wrong, given))| {
            let most = given.values().copied().max();
            ValueTail {
                value,
                decisions: n,
                answered_otherwise: wrong as f64 / n as f64,
                given_instead: most
                    .and_then(|m| given.iter().find(|(_, c)| **c == m))
                    .map(|(v, _)| v.clone()),
            }
        })
        .collect()
}

#[test]
fn the_report_counts_what_the_chosen_thresholds_answered_value_by_value() {
    let records = records(2_000);
    let mut decisions = Vec::new();
    let with = chosen(&records, true, Some(&mut |d| decisions.push(d.clone())));
    let tails = with.value_tails.clone().expect("asked for");

    assert_eq!(tails, by_hand(&decisions, &with));
    assert!(tails.iter().all(|t| t.decisions >= VALUE_TAIL_DECISIONS));
    let security = tails
        .iter()
        .find(|t| t.value == "security")
        .expect("reported");
    assert_eq!(security.given_instead.as_deref(), Some("network"));
    for other in tails.iter().filter(|t| t.value != "security") {
        assert!(
            security.answered_otherwise > other.answered_otherwise + 0.3,
            "{security:?} against {other:?}"
        );
    }
}

#[test]
fn asking_for_the_report_changes_nothing_else_chosen() {
    let records = records(1_200);
    let without = chosen(&records, false, None);
    let with = chosen(&records, true, None);

    assert_eq!(without.value_tails, None);
    assert!(with.value_tails.is_some());
    assert_eq!(
        LayerThresholds {
            value_tails: None,
            ..with
        },
        without
    );
}

#[test]
fn values_settled_too_rarely_are_left_out() {
    let records = records(60);
    let tails = chosen(&records, true, None).value_tails.expect("asked for");
    assert!(tails.iter().all(|t| t.decisions >= VALUE_TAIL_DECISIONS));
    assert!(!tails.iter().any(|t| t.value == "security"));
}

#[cfg(feature = "serde")]
#[test]
fn the_report_round_trips_and_is_left_out_when_not_asked_for() {
    let records = records(800);
    let without = serde_json::to_value(chosen(&records, false, None)).unwrap();
    assert!(without.get("value_tails").is_none());

    let with = chosen(&records, true, None);
    let text = serde_json::to_string(&with).unwrap();
    assert_eq!(
        serde_json::from_str::<LayerThresholds>(&text).unwrap(),
        with
    );
}
