//! The settled key memory, held to what the reference implementation's key memory, latest settlement and must-catch tests
//! expect: which attributes become keys when learning and when ranking, that a record contributes once however often
//! and however piecemeal it is put, that the order records arrive in never changes a score, how recent settlements
//! outweigh older ones, and that a key which once settled a value to catch is no longer trusted.

use std::collections::BTreeMap;

use gil::decider::{Answer, Decider};
use gil::key_memory::{Candidate, KeyMemory, Lookup};
use gil::lexical_memory::LexicalMemory;
use gil::schema::{Attribute, AttributeRole, CoarseLevel, Record, Schema};

const MINUTE: i64 = 60 * 10_000_000;
const DAY: i64 = 24 * 60 * MINUTE;

fn attribute(name: &str, role: AttributeRole) -> Attribute {
    Attribute::new(name, role)
}

/// A reporter that is no evidence, two observed attributes, and two decided ones.
fn ticket() -> Schema {
    let mut reporter = attribute("reporter", AttributeRole::Observed);
    reporter.use_as_evidence = false;
    let mut severity = attribute("severity", AttributeRole::Decided);
    severity.candidates = Some(vec!["low".into(), "medium".into(), "high".into()]);
    Schema {
        name: "ticket".into(),
        attributes: vec![
            reporter,
            attribute("component", AttributeRole::Observed),
            attribute("summary", AttributeRole::Observed),
            severity,
            attribute("team", AttributeRole::Decided),
        ],
    }
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(a, v)| ((*a).to_owned(), (*v).to_owned()))
        .collect()
}

/// Record `d<n>`, settled `n` minutes after the epoch: records settle in the order of their numbers, a minute apart.
fn doc(number: i64, pairs: &[(&str, &str)]) -> Record {
    Record {
        id: format!("d{number}"),
        values: values(pairs),
        settled_at: number * MINUTE,
        ..Record::default()
    }
}

fn rank(
    memory: &KeyMemory,
    schema: &Schema,
    attribute: &str,
    known: &[(&str, &str)],
    count: usize,
) -> Vec<Candidate> {
    let known = values(known);
    memory.rank(&Lookup::new(schema, attribute, &known), count)
}

fn ranked_values(candidates: &[Candidate]) -> Vec<&str> {
    candidates.iter().map(|c| c.value.as_str()).collect()
}

#[test]
fn an_attribute_that_is_not_evidence_never_becomes_a_key() {
    let schema = ticket();
    let mut memory = KeyMemory::new(60, 1.0);
    memory.put(
        &schema,
        &doc(1, &[("reporter", "Kim"), ("severity", "high")]),
    );
    memory.put(
        &schema,
        &doc(2, &[("reporter", "Lee"), ("severity", "low")]),
    );
    memory.put(
        &schema,
        &doc(3, &[("reporter", "Lee"), ("severity", "low")]),
    );

    // With the reporter as a key, "Kim" would point to high; without it only the overall frequency is left.
    let ranked = rank(&memory, &schema, "severity", &[("reporter", "Kim")], 1);

    assert_eq!(
        ranked,
        [Candidate {
            value: "low".into(),
            score: 2.0 / 3.0,
            evidence: None,
            trusted: false,
        }]
    );
}

#[test]
fn ranking_reads_only_the_known_attributes_but_learning_takes_every_other_attribute_as_a_key() {
    let schema = ticket();
    let mut memory = KeyMemory::default();
    // Learned together: team is a key for severity although it would be settled after it.
    memory.put(
        &schema,
        &doc(
            1,
            &[
                ("component", "vpn"),
                ("severity", "high"),
                ("team", "network"),
            ],
        ),
    );
    memory.put(
        &schema,
        &doc(
            2,
            &[
                ("component", "printer"),
                ("severity", "low"),
                ("team", "facilities"),
            ],
        ),
    );

    assert_eq!(
        ranked_values(&rank(
            &memory,
            &schema,
            "severity",
            &[("team", "network")],
            1
        )),
        ["high"]
    );
    assert_eq!(
        ranked_values(&rank(
            &memory,
            &schema,
            "severity",
            &[("component", "printer")],
            1
        )),
        ["low"]
    );
}

#[test]
fn putting_a_record_again_replaces_its_contribution_so_it_never_counts_twice() {
    let schema = ticket();
    let mut memory = KeyMemory::default();
    memory.put(
        &schema,
        &doc(1, &[("component", "vpn"), ("severity", "low")]),
    );
    memory.put(
        &schema,
        &doc(1, &[("component", "vpn"), ("severity", "high")]),
    ); // corrected later
    memory.put(
        &schema,
        &doc(1, &[("component", "vpn"), ("severity", "high")]),
    ); // and put again unchanged

    assert_eq!(memory.count("ticket"), 1);
    let ranked = rank(&memory, &schema, "severity", &[("component", "vpn")], 3);
    let scored: Vec<(&str, f64)> = ranked.iter().map(|c| (c.value.as_str(), c.score)).collect();
    assert_eq!(scored, [("high", 0.5)]); // one settlement: 1 / (1 + 1)

    memory.remove("ticket", "d1");
    assert_eq!(memory.count("ticket"), 0);
    assert!(rank(&memory, &schema, "severity", &[("component", "vpn")], 3).is_empty());
}

#[test]
fn settling_attribute_by_attribute_reaches_the_same_state_as_putting_the_finished_records_in_reverse()
 {
    let schema = ticket();
    let finished: Vec<(i64, Vec<(&str, &str)>)> = vec![
        (
            1,
            vec![
                ("component", "vpn"),
                ("severity", "high"),
                ("team", "network"),
            ],
        ),
        (
            2,
            vec![
                ("component", "printer"),
                ("severity", "low"),
                ("team", "facilities"),
            ],
        ),
        (
            3,
            vec![
                ("component", "vpn"),
                ("severity", "medium"),
                ("team", "network"),
            ],
        ),
        (
            4,
            vec![
                ("component", "disk"),
                ("severity", "high"),
                ("team", "storage"),
            ],
        ),
    ];

    let mut live = KeyMemory::default();
    for (number, pairs) in &finished {
        // Values arrive one at a time and the record is put after each, as an open record does.
        for arrived in 1..=pairs.len() {
            live.put(&schema, &doc(*number, &pairs[..arrived]));
        }
    }

    let mut rebuilt = KeyMemory::default();
    for (number, pairs) in finished.iter().rev() {
        rebuilt.put(&schema, &doc(*number, pairs));
    }

    for known in [
        [("component", "vpn")],
        [("team", "storage")],
        [("component", "keyboard")],
    ] {
        for attribute in ["severity", "team"] {
            assert_eq!(
                rank(&live, &schema, attribute, &known, 3),
                rank(&rebuilt, &schema, attribute, &known, 3),
                "{attribute} given {known:?}"
            );
        }
    }
}

/// A small deterministic generator (64-bit linear congruential), so the long history is the same on every run.
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

#[test]
fn a_long_history_weighs_the_same_to_the_bit_whatever_order_it_was_put_in() {
    // Seven thousand settlements under one key, put in the order they were settled and shuffled: both must agree to
    // the bit, past the point where the earliest weights would no longer fit a double unscaled.
    let schema = ticket();
    let mut random = Lcg(11);
    let severities = ["low", "medium", "high"];
    let records: Vec<Record> = (1..=7000)
        .map(|i| {
            let choices = if i < 6000 { 3 } else { 2 };
            let severity = severities[random.below(choices) as usize];
            doc(i, &[("component", "vpn"), ("severity", severity)])
        })
        .collect();

    let mut in_order = KeyMemory::default();
    for record in &records {
        in_order.put(&schema, record);
    }

    let mut shuffled: Vec<&Record> = records.iter().collect();
    for i in (1..shuffled.len()).rev() {
        let j = random.below(i as u64 + 1) as usize;
        shuffled.swap(i, j);
    }
    let mut afresh = KeyMemory::default();
    for record in shuffled {
        afresh.put(&schema, record);
    }

    let known = [("component", "vpn")];
    let carried = rank(&in_order, &schema, "severity", &known, 3);
    assert_eq!(carried, rank(&afresh, &schema, "severity", &known, 3));
    assert_eq!(carried.len(), 3);
    assert_ne!(carried[0].value, "high"); // the last thousand never settled it
}

#[test]
fn a_schema_with_another_name_has_an_empty_memory() {
    let schema = ticket();
    let mut memory = KeyMemory::default();
    memory.put(
        &schema,
        &doc(1, &[("component", "vpn"), ("severity", "high")]),
    );
    let elsewhere = Schema {
        name: "elsewhere".into(),
        ..schema.clone()
    };

    assert_eq!(memory.count("ticket"), 1);
    assert_eq!(memory.count("elsewhere"), 0);
    assert!(rank(&memory, &elsewhere, "severity", &[("component", "vpn")], 3).is_empty());
    assert_eq!(
        rank(
            &memory,
            &schema.clone(),
            "severity",
            &[("component", "vpn")],
            3
        )
        .len(),
        1
    );
}

/// An observed summary and a decided team.
fn support() -> Schema {
    let mut team = attribute("team", AttributeRole::Decided);
    team.memory_threshold = Some(0.3);
    Schema {
        name: "ticket".into(),
        attributes: vec![attribute("summary", AttributeRole::Observed), team],
    }
}

fn settled(id: &str, summary: &str, team: &str, at: i64) -> Record {
    Record {
        id: id.into(),
        values: values(&[("summary", summary), ("team", team)]),
        settled_at: at,
        ..Record::default()
    }
}

#[test]
fn values_settled_equally_often_rank_by_the_latest_settlement_not_by_arrival() {
    let schema = support();
    let older = settled("b", "vpn drops", "facilities", 0);
    let newer = settled("a", "vpn drops", "network", DAY);

    for order in [[&older, &newer], [&newer, &older]] {
        let mut memory = KeyMemory::default();
        for record in order {
            memory.put(&schema, record);
        }

        // Tied one to one under the key and overall; the scalar order alone would put "facilities" first.
        assert_eq!(
            ranked_values(&rank(
                &memory,
                &schema,
                "team",
                &[("summary", "vpn drops")],
                3
            )),
            ["network", "facilities"]
        );
        assert_eq!(
            ranked_values(&rank(&memory, &schema, "team", &[], 3)),
            ["network", "facilities"]
        );
    }
}

#[test]
fn weighing_by_recency_follows_settlement_times_not_arrival_order() {
    let schema = support();
    let records: Vec<Record> = (0..12)
        .map(|i| {
            settled(
                &format!("d{i:02}"),
                if i % 3 == 0 { "vpn drops" } else { "vpn slow" },
                if i < 7 { "facilities" } else { "network" },
                i * 60 * MINUTE,
            )
        })
        .collect();
    let reversed: Vec<Record> = records.iter().rev().cloned().collect();
    // Evens first, then odds — neither the settlement order nor its reverse.
    let interleaved: Vec<Record> = records
        .iter()
        .step_by(2)
        .chain(records.iter().skip(1).step_by(2))
        .cloned()
        .collect();

    let ranked: Vec<Vec<Candidate>> = [records, reversed, interleaved]
        .iter()
        .map(|order| {
            let mut memory = KeyMemory::default();
            for record in order {
                memory.put(&schema, record);
            }
            rank(&memory, &schema, "team", &[("summary", "vpn drops")], 3)
        })
        .collect();

    assert!(!ranked[0].is_empty());
    assert_eq!(ranked[1], ranked[0]);
    assert_eq!(ranked[2], ranked[0]);
}

/// Twenty settlements of "facilities", then corrections to "network" one at a time: the first place turns to
/// "network" at exactly the `needed`-th correction.
fn correction_overtakes_after(decay: f64, needed: i64) {
    let schema = support();
    let mut memory = KeyMemory::new(60, decay);
    let known = [("summary", "vpn drops")];
    for i in 0..20 {
        memory.put(
            &schema,
            &settled(&format!("old{i:02}"), "vpn drops", "facilities", i * MINUTE),
        );
    }

    for k in 1..=needed {
        memory.put(
            &schema,
            &settled(
                &format!("new{k:02}"),
                "vpn drops",
                "network",
                DAY + k * MINUTE,
            ),
        );
        let first = &rank(&memory, &schema, "team", &known, 1)[0].value;
        let expected = if k < needed { "facilities" } else { "network" };
        assert_eq!(first, expected, "after {k} corrections");
    }
}

#[test]
fn plain_counting_lets_a_correction_overtake_after_as_many_settlements_and_the_tie_goes_to_the_latest()
 {
    correction_overtakes_after(1.0, 20);
}

#[test]
fn a_decay_of_095_lets_a_correction_overtake_after_ten_settlements() {
    // 0.95^k < 1 / (2 − 0.95^20) first holds at k = 10.
    correction_overtakes_after(0.95, 10);
}

#[test]
#[should_panic(expected = "recency_decay")]
fn a_decay_of_zero_is_rejected() {
    let _ = KeyMemory::new(60, 0.0);
}

#[test]
#[should_panic(expected = "recency_decay")]
fn a_decay_above_one_is_rejected() {
    let _ = KeyMemory::new(60, 1.01);
}

/// An observed template and a decided risk trusted from 0.5, with the values it must not miss.
fn log(must_catch: Option<&[&str]>) -> Schema {
    let mut risk = attribute("risk", AttributeRole::Decided);
    risk.key_threshold = Some(0.5);
    risk.must_catch = must_catch.map(|m| m.iter().map(|v| (*v).to_owned()).collect());
    Schema {
        name: "log".into(),
        attributes: vec![attribute("template", AttributeRole::Observed), risk],
    }
}

fn line(number: i64, template: &str, risk: &str) -> Record {
    Record {
        id: format!("l{number}"),
        values: values(&[("template", template), ("risk", risk)]),
        settled_at: number * MINUTE,
        ..Record::default()
    }
}

/// Under t1 a 3 was settled once among many 1s; under t2 only 1s.
fn lines() -> Vec<Record> {
    (1..=20)
        .map(|n| line(n, "t1", "1"))
        .chain([line(21, "t1", "3")])
        .chain((22..=41).map(|n| line(n, "t1", "1")))
        .chain((42..=51).map(|n| line(n, "t2", "1")))
        .collect()
}

fn ranked_risk(schema: &Schema, template: &str) -> Vec<Candidate> {
    let mut memory = KeyMemory::default();
    for line in lines() {
        memory.put(schema, &line);
    }
    rank(&memory, schema, "risk", &[("template", template)], 3)
}

#[test]
fn a_key_that_once_settled_a_value_to_catch_is_not_trusted_but_still_offers_it() {
    let caught = ranked_risk(&log(Some(&["3", "4", "5"])), "t1");
    assert_eq!(caught[0].value, "1");
    assert!(!caught[0].trusted); // still offered, as a guess
    assert!(ranked_values(&caught).contains(&"3"));

    // A key that never settled one answers as before.
    assert!(ranked_risk(&log(Some(&["3", "4", "5"])), "t2")[0].trusted);
    // The threshold alone lets the rare 3 through.
    assert!(ranked_risk(&log(None), "t1")[0].trusted);
    assert!(ranked_risk(&log(Some(&[])), "t1")[0].trusted);
}

/// As [`log`], with the text each line says: lines under one template read alike, so the similar record layer, trusted
/// at any score, answers wherever the key layer does not.
fn written(must_catch: Option<&[&str]>) -> Schema {
    let mut risk = attribute("risk", AttributeRole::Decided);
    risk.key_fields = Some(vec!["template".into()]);
    risk.key_threshold = Some(0.5);
    risk.memory_threshold = Some(0.0);
    risk.must_catch = must_catch.map(|m| m.iter().map(|v| (*v).to_owned()).collect());
    risk.coarse = Some(CoarseLevel {
        classes: Some(BTreeMap::from([
            ("1".to_owned(), "low".to_owned()),
            ("3".to_owned(), "high".to_owned()),
        ])),
        key_threshold: Some(0.0),
        memory_threshold: Some(0.0),
        ..CoarseLevel::default()
    });
    Schema {
        name: "log".into(),
        attributes: vec![
            attribute("template", AttributeRole::Observed),
            attribute("message", AttributeRole::Observed),
            risk,
        ],
    }
}

fn written_answer(schema: &Schema, template: &str) -> Answer {
    let written: Vec<Record> = lines()
        .into_iter()
        .map(|mut line| {
            let message = format!("disk check {} finished", line.values["template"]);
            line.values.insert("message".into(), message);
            line
        })
        .collect();
    let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    decider.rebuild(schema, &written);
    let message = format!("disk check {template} finished");
    let asked = values(&[("template", template), ("message", &message)]);
    decider
        .answer(schema, "l99", &asked, &BTreeMap::new())
        .into_iter()
        .find(|a| a.attribute == "risk")
        .expect("risk is open")
}

#[test]
fn a_record_under_a_key_that_settled_a_value_to_catch_is_not_answered_by_a_similar_record_either() {
    let caught = written_answer(&written(Some(&["3"])), "t1");
    // The similar records, nearly all saying 1, do not answer where the key may not.
    assert!(!caught.answered());
    assert!(caught.candidates.iter().all(|c| !c.trusted));
    assert!(caught.candidates.iter().any(|c| c.value == "3"));
    assert!(caught.coarse.is_none());

    assert!(written_answer(&written(Some(&["3"])), "t2").answered());
    let plain = written_answer(&written(None), "t1");
    assert!(plain.answered());
    assert_eq!(plain.candidates[0].value, "1");
}
