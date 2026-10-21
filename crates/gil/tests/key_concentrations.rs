//! Key concentrations — how readily each key attribute's keys are believed after only a few settlements — held to what the
//! reference implementation does: a concentration of 1 ranks exactly as none, the choice does not depend on the order the
//! records are given in, a key never seen again chooses nothing and a key seen again once with its value moves only part of
//! the way from 1, an attribute that takes several values gets no concentrations, an observed attribute is not chosen for,
//! and a concentration for an unknown attribute or not above 0 is refused by the schema check.

use std::collections::BTreeMap;

use gil::key_memory::{KeyMemory, Lookup};
use gil::replay::select_key_concentrations;
use gil::schema::{Attribute, AttributeRole, Record, Schema};

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

/// Each place has one risk, drawn mostly high; each visit has a kind drawn from a few. The kind does not decide the risk —
/// its keys mix all three — while a place seen once already names it.
fn inspection(concentrations: &[(&str, f64)]) -> Schema {
    try_inspection(concentrations).expect("a valid schema")
}

fn try_inspection(concentrations: &[(&str, f64)]) -> Result<Schema, String> {
    let mut risk = Attribute::new("risk", AttributeRole::Decided);
    risk.key_concentrations = concentrations
        .iter()
        .map(|(k, c)| ((*k).to_owned(), *c))
        .collect();
    Schema::new(
        "inspection",
        vec![
            Attribute::new("place", AttributeRole::Observed),
            Attribute::new("kind", AttributeRole::Observed),
            risk,
        ],
    )
}

fn record(i: usize, values: &[(&str, &str)]) -> Record {
    Record {
        id: format!("d{i}"),
        values: values
            .iter()
            .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
            .collect(),
        settled_at: i as i64 * MINUTE,
        ..Record::default()
    }
}

fn inspections(count: usize, seed: u64) -> Vec<Record> {
    let mut draws = Draws(seed);
    let risk: Vec<usize> = (0..150)
        .map(|_| match draws.unit() {
            u if u < 0.75 => 0,
            u if u < 0.95 => 1,
            _ => 2,
        })
        .collect();
    (0..count)
        .map(|i| {
            let place = draws.below(risk.len());
            let kind = draws.below(4);
            record(
                i,
                &[
                    ("place", &format!("p{place}")),
                    ("kind", &format!("k{kind}")),
                    ("risk", RISKS[risk[place]]),
                ],
            )
        })
        .collect()
}

fn known(place: &str, kind: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("place".to_owned(), place.to_owned()),
        ("kind".to_owned(), kind.to_owned()),
    ])
}

fn chosen(schema: &Schema, attribute: &str, records: &[Record]) -> BTreeMap<String, f64> {
    select_key_concentrations(&mut KeyMemory::default(), schema, attribute, records)
}

#[test]
fn concentrations_of_one_rank_exactly_as_none() {
    let mut memory = KeyMemory::default();
    for r in &inspections(300, 3) {
        memory.put(&inspection(&[]), r);
    }

    let none = inspection(&[]);
    let ones = inspection(&[("place", 1.0), ("kind", 1.0)]);
    for (place, kind) in [("p3", "k1"), ("p40", "k2"), ("p149", "k0")] {
        let known = known(place, kind);
        let without = memory.rank(&Lookup::new(&none, "risk", &known), 3);
        let with = memory.rank(&Lookup::new(&ones, "risk", &known), 3);
        assert!(!without.is_empty(), "{place}/{kind} has candidates");
        assert_eq!(with, without, "{place}/{kind}");
    }
}

#[test]
fn the_same_records_in_any_order_give_the_same_concentrations() {
    let records = inspections(400, 3);
    let reversed: Vec<Record> = records.iter().rev().cloned().collect();

    let forward = chosen(&inspection(&[]), "risk", &records);
    let backward = chosen(&inspection(&[]), "risk", &reversed);

    assert_eq!(
        forward.keys().map(String::as_str).collect::<Vec<_>>(),
        ["kind", "place"]
    );
    assert_eq!(backward, forward);
}

#[test]
fn without_a_key_seen_again_no_concentration_is_chosen() {
    let once = record(0, &[("place", "p0"), ("kind", "k0"), ("risk", "high")]);

    assert!(chosen(&inspection(&[]), "risk", &[once]).is_empty());
}

#[test]
fn a_key_seen_again_once_with_its_value_moves_only_part_of_the_way_from_one() {
    // One prediction of the value a key had, beside the two anchoring ones: (0 + 1) / α = 1 / (1 + α) + 2 / (1 + α),
    // so α = 0.5.
    let records = [
        record(0, &[("place", "p0"), ("risk", "high")]),
        record(1, &[("place", "p0"), ("risk", "high")]),
    ];

    let found = chosen(&inspection(&[]), "risk", &records);

    assert_eq!(
        found.keys().map(String::as_str).collect::<Vec<_>>(),
        ["place"]
    );
    assert!((found["place"] - 0.5).abs() < 1e-12, "{}", found["place"]);
}

#[test]
fn an_attribute_that_takes_several_values_gets_no_concentrations() {
    let mut tags = Attribute::new("tags", AttributeRole::Decided);
    tags.multiple = true;
    let schema = Schema::new(
        "tags",
        vec![Attribute::new("place", AttributeRole::Observed), tags],
    )
    .expect("a valid schema");
    let records: Vec<Record> = (0..5)
        .map(|i| Record {
            sets: BTreeMap::from([("tags".to_owned(), vec!["a".to_owned(), "b".to_owned()])]),
            ..record(i, &[("place", "p0")])
        })
        .collect();

    assert!(chosen(&schema, "tags", &records).is_empty());
}

#[test]
#[should_panic(expected = "is not a decided attribute")]
fn an_observed_attribute_is_not_chosen_for() {
    chosen(&inspection(&[]), "place", &inspections(10, 3));
}

#[test]
fn a_concentration_for_an_unknown_attribute_or_not_above_zero_is_refused() {
    assert!(try_inspection(&[("place", 0.5), ("kind", 2.0)]).is_ok());
    for (key, concentration) in [
        ("nowhere", 0.5),
        ("place", 0.0),
        ("place", -1.0),
        ("place", f64::NAN),
        ("place", f64::INFINITY),
    ] {
        assert!(
            try_inspection(&[(key, concentration)]).is_err(),
            "{key}: {concentration}"
        );
    }
}
