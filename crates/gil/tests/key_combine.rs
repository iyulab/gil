//! Key combination — how a value's strengths under the record's several keys make its score. Summing is the default and
//! ranks exactly as an attribute that names none; taking the strongest leaves an attribute's ranking, and the key threshold
//! a replay chooses for it, where they were when another attribute repeats one of its keys, while summing moves them; the
//! strongest is the score under the strongest key; and a schema that sums writes no setting, one that takes the strongest
//! writes and reads it back.

use std::collections::BTreeMap;

use gil::key_memory::{KeyMemory, Lookup};
use gil::replay::{Choice, select_key_threshold};
use gil::schema::{Attribute, AttributeRole, KeyCombine, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;
const RISKS: [&str; 3] = ["high", "medium", "low"];

/// A small deterministic generator (SplitMix64), so the records are the same on every run.
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
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Places with one risk each and visits of a few kinds; with `repeated`, an attribute that copies the place.
fn inspection(combine: Option<KeyCombine>, repeated: bool) -> Schema {
    let mut risk = Attribute::new("risk", AttributeRole::Decided);
    if let Some(combine) = combine {
        risk.key_combine = combine;
    }
    let mut attributes = vec![
        Attribute::new("place", AttributeRole::Observed),
        Attribute::new("kind", AttributeRole::Observed),
    ];
    if repeated {
        attributes.push(Attribute::new("site", AttributeRole::Observed));
    }
    attributes.push(risk);
    Schema::new("inspection", attributes).expect("a valid schema")
}

fn inspections(count: usize, seed: u64, repeated: bool) -> Vec<Record> {
    let mut draws = Draws(seed);
    let risk: Vec<usize> = (0..60)
        .map(|_| match draws.unit() {
            u if u < 0.6 => 0,
            u if u < 0.9 => 1,
            _ => 2,
        })
        .collect();
    (0..count)
        .map(|i| {
            let place = draws.below(risk.len());
            let kind = draws.below(4);
            // The risk follows the place most of the time.
            let settled = if draws.unit() < 0.85 {
                risk[place]
            } else {
                draws.below(3)
            };
            let mut values: BTreeMap<String, String> = BTreeMap::from([
                ("place".to_owned(), format!("p{place}")),
                ("kind".to_owned(), format!("k{kind}")),
                ("risk".to_owned(), RISKS[settled].to_owned()),
            ]);
            if repeated {
                values.insert("site".to_owned(), format!("p{place}"));
            }
            Record {
                id: format!("d{i}"),
                values,
                settled_at: i as i64 * MINUTE,
                ..Record::default()
            }
        })
        .collect()
}

fn known(place: &str, kind: &str, repeated: bool) -> BTreeMap<String, String> {
    let mut known = BTreeMap::from([
        ("place".to_owned(), place.to_owned()),
        ("kind".to_owned(), kind.to_owned()),
    ]);
    if repeated {
        known.insert("site".to_owned(), place.to_owned());
    }
    known
}

/// The value, score and backing key of each of the first three candidates for a few records.
fn ranked(combine: Option<KeyCombine>, repeated: bool) -> Vec<Vec<(String, f64, Option<String>)>> {
    let schema = inspection(combine, repeated);
    let mut memory = KeyMemory::default();
    for r in &inspections(400, 5, repeated) {
        memory.put(&schema, r);
    }
    [("p3", "k1"), ("p17", "k2"), ("p59", "k0")]
        .iter()
        .map(|(place, kind)| {
            let known = known(place, kind, repeated);
            memory
                .rank(&Lookup::new(&schema, "risk", &known), 3)
                .into_iter()
                // The copy backs a value exactly as strongly as the place; the key named is the first of them either way.
                .map(|c| (c.value, c.score, c.evidence.map(|k| k.attribute)))
                .collect()
        })
        .collect()
}

#[test]
fn summing_is_the_default_and_ranks_as_naming_none() {
    assert_eq!(KeyCombine::default(), KeyCombine::Sum);
    assert_eq!(
        Attribute::new("risk", AttributeRole::Decided).key_combine,
        KeyCombine::Sum
    );
    assert_eq!(ranked(Some(KeyCombine::Sum), false), ranked(None, false));
}

#[test]
fn taking_the_strongest_leaves_the_ranking_where_it_was_when_an_attribute_repeats_a_key() {
    let alone = ranked(Some(KeyCombine::Max), false);
    let repeated = ranked(Some(KeyCombine::Max), true);
    assert!(alone.iter().all(|c| !c.is_empty()));
    assert_eq!(repeated, alone);
}

#[test]
fn summing_moves_the_ranking_when_an_attribute_repeats_a_key() {
    let alone = ranked(None, false);
    let repeated = ranked(None, true);
    assert_ne!(repeated, alone);
}

#[test]
fn the_strongest_is_the_score_under_the_strongest_key() {
    let schema = inspection(Some(KeyCombine::Max), false);
    let mut memory = KeyMemory::default();
    for r in &inspections(400, 5, false) {
        memory.put(&schema, r);
    }
    let known = known("p3", "k1", false);
    let lookup = Lookup::new(&schema, "risk", &known);
    for candidate in memory
        .rank(&lookup, 3)
        .iter()
        .filter(|c| c.evidence.is_some())
    {
        let strongest = memory
            .key_totals(&lookup, &candidate.value)
            .into_iter()
            .map(|(attribute, total, own)| {
                own / (total + schema.attribute("risk").concentration(&attribute))
            })
            .fold(0.0, f64::max);
        assert!(
            (candidate.score - strongest).abs() < 1e-12,
            "{}: {} against {strongest}",
            candidate.value,
            candidate.score
        );
    }
}

#[test]
fn a_replay_taking_the_strongest_chooses_the_same_key_threshold_when_an_attribute_repeats_a_key() {
    let choose = |combine: Option<KeyCombine>, repeated: bool| {
        let replay = select_key_threshold(
            &mut KeyMemory::default(),
            &inspection(combine, repeated),
            "risk",
            &inspections(400, 5, repeated),
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: None,
            },
        );
        let chosen = replay.chosen.expect("a threshold is chosen");
        (chosen.threshold, chosen.answered, replay.lookups)
    };

    assert_eq!(
        choose(Some(KeyCombine::Max), true),
        choose(Some(KeyCombine::Max), false)
    );
    assert_ne!(choose(None, true), choose(None, false));
}

#[cfg(feature = "serde")]
#[test]
fn a_summing_schema_writes_no_setting_and_the_strongest_comes_back() {
    let summing = serde_json::to_string(&inspection(None, false)).expect("serialized");
    assert!(!summing.contains("key_combine"), "{summing}");

    let strongest =
        serde_json::to_string(&inspection(Some(KeyCombine::Max), false)).expect("serialized");
    assert!(strongest.contains(r#""key_combine":"max""#), "{strongest}");
    let back: Schema = serde_json::from_str(&strongest).expect("deserialized");
    assert_eq!(back.attribute("risk").key_combine, KeyCombine::Max);
}
