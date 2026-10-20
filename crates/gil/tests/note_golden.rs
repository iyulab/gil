//! The note golden (`tests/fixtures/core/note-golden.jsonl`), written by the .NET implementation on the made-up notes of
//! `note-logs.jsonl`: a schema whose long free text decides a decided attribute through the similar record layer, with a
//! value that must not be missed, a value settled outside the attribute's candidates, a coarse level made from a table, a
//! attribute that takes several values, values arriving in different orders, and records settled again and copied while
//! the resolver is asked. The settings it chose and its decisions — before typing, with a set attribute's first value
//! chosen, with the owner's first characters typed, and asked again once saved — must come out the same here, to
//! the bit.

mod common;

use std::collections::BTreeMap;

use common::{doubles, fixture, lines, replay, to_record};
use gil::decider::{Answer, Candidate, Decider};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, select_coarse, select_key_threshold, select_layers, select_typed_key_thresholds,
};
use gil::schema::{Attribute, AttributeRole, CoarseLevel, Record, Schema};
use serde_json::{Value, json};

const CHOSEN: usize = 240;

const TOPICS: [&str; 7] = [
    "net-wifi",
    "net-cable",
    "pay-card",
    "pay-bank",
    "acct-login",
    "acct-delete",
    "fraud",
];

/// The note schema with the settings the golden chose before the first line `stop` holds for.
fn note_form(golden: &str, stop: impl Fn(&Value) -> bool) -> Schema {
    let mut topic = Attribute::new("topic", AttributeRole::Decided);
    topic.depends_on = Some(vec!["body".to_owned(), "ref".to_owned()]);
    topic.candidates = Some(TOPICS.iter().map(|t| (*t).to_owned()).collect());
    topic.must_catch = Some(vec!["fraud".to_owned()]);
    topic.coarse = Some(CoarseLevel {
        classes: Some(BTreeMap::from(
            [
                ("net-wifi", "network"),
                ("net-cable", "network"),
                ("pay-card", "payment"),
                ("pay-bank", "payment"),
                ("acct-login", "account"),
                ("acct-delete", "account"),
            ]
            .map(|(v, c)| (v.to_owned(), c.to_owned())),
        )),
        ..CoarseLevel::default()
    });
    let mut labels = Attribute::new("labels", AttributeRole::Decided);
    labels.multiple = true;
    labels.depends_on = Some(vec!["desk".to_owned()]);
    let mut owner = Attribute::new("owner", AttributeRole::Decided);
    owner.depends_on = Some(vec!["desk".to_owned(), "ref".to_owned()]);

    let chosen = |part: &Value, name: &str| part["chosen"][name].as_f64();
    for line in
        lines(golden).take_while(|l| !l["kind"].as_str().unwrap().contains("decision") && !stop(l))
    {
        let target = match line["field"].as_str() {
            Some("topic") => &mut topic,
            Some("labels") => &mut labels,
            Some("owner") => &mut owner,
            _ => continue,
        };
        match line["kind"].as_str() {
            Some("layers") => {
                target.key_threshold = chosen(&line["key"], "threshold");
                target.memory_threshold = chosen(&line["memory"], "threshold");
                target.memory_floor = chosen(&line["memory"], "similarity_floor");
            }
            Some("coarse") => {
                let level = target.coarse.as_mut().expect("a coarse level");
                level.key_threshold = chosen(&line["key"], "threshold");
                level.memory_threshold = chosen(&line["memory"], "threshold");
                level.memory_floor = chosen(&line["memory"], "similarity_floor");
            }
            Some("key") => target.key_threshold = chosen(&line["replay"], "threshold"),
            Some("typed") => {
                target.typed_key_thresholds = Some(
                    line["replays"]
                        .as_array()
                        .expect("replays")
                        .iter()
                        .map(|r| chosen(r, "threshold"))
                        .collect(),
                );
            }
            _ => {}
        }
    }

    Schema {
        name: "note".to_owned(),
        attributes: vec![
            Attribute::new("desk", AttributeRole::Observed),
            Attribute::new("body", AttributeRole::Observed),
            Attribute::new("ref", AttributeRole::Observed),
            topic,
            labels,
            owner,
        ],
    }
}

fn notes() -> Vec<Record> {
    lines(&fixture("note-records.jsonl"))
        .map(|r| to_record(&r))
        .collect()
}

#[test]
fn the_note_settings_reproduce_their_golden() {
    let records = notes();
    let early = &records[..CHOSEN];
    let golden = fixture("note-golden.jsonl");
    let counted = Some(&common::COUNTED[..]);
    for expected in lines(&golden)
        .filter(|l| ["layers", "coarse", "key", "typed"].contains(&l["kind"].as_str().unwrap()))
    {
        let (kind, attribute) = (
            expected["kind"].as_str().unwrap().to_owned(),
            expected["field"].as_str().unwrap().to_owned(),
        );
        let (kind, attribute) = (kind.as_str(), attribute.as_str());
        let schema = note_form(&golden, |l| l["kind"] == kind && l["field"] == attribute);
        let actual = match kind {
            "layers" | "coarse" => {
                let choose = if kind == "layers" {
                    select_layers
                } else {
                    select_coarse
                };
                let layers = choose(
                    &mut KeyMemory::default(),
                    &mut LexicalMemory::default(),
                    &schema,
                    attribute,
                    early,
                    Choice {
                        target_precision: 0.8,
                        minimum_answered: 10,
                        counting: counted,
                    },
                );
                json!({ "kind": kind, "field": attribute, "key": replay(&layers.key), "memory": replay(&layers.memory) })
            }
            "typed" => {
                let typed = select_typed_key_thresholds(
                    &mut KeyMemory::default(),
                    &schema,
                    attribute,
                    early,
                    Choice {
                        target_precision: 0.8,
                        minimum_answered: 10,
                        counting: counted,
                    },
                    2,
                );
                json!({ "kind": kind, "field": attribute, "replays": typed.by_length.iter().map(replay).collect::<Vec<_>>() })
            }
            _ => {
                let replay = select_key_threshold(
                    &mut KeyMemory::default(),
                    &schema,
                    attribute,
                    early,
                    Choice {
                        target_precision: 0.8,
                        minimum_answered: 10,
                        counting: None,
                    },
                );
                json!({ "kind": kind, "field": attribute, "replay": self::replay(&replay) })
            }
        };
        assert_eq!(doubles(actual), doubles(expected), "{kind} {attribute}");
    }
}

fn candidate(c: &Candidate) -> Value {
    json!([
        c.value,
        c.score,
        c.source.name(),
        c.evidence.as_ref().map(ToString::to_string),
        c.trusted
    ])
}

fn decision(kind: &str, id: &str, s: &Answer) -> Value {
    json!({
        "kind": kind,
        "doc": id,
        "field": s.attribute,
        "answered": s.answered(),
        "source": s.source.name(),
        "candidates": s.candidates.iter().map(candidate).collect::<Vec<_>>(),
        "coarse": s.coarse.as_ref().map(candidate),
    })
}

#[test]
fn the_note_decisions_reproduce_their_golden() {
    let records = notes();
    let early = &records[..CHOSEN];
    let golden = fixture("note-golden.jsonl");
    let schema = note_form(&golden, |_| false);
    let expected: Vec<Value> = lines(&golden)
        .filter(|l| l["kind"].as_str().unwrap().ends_with("decision"))
        .collect();

    let mut resolver = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&schema, early);
    let minute = 60 * 10_000_000;
    let mut latest = records.last().unwrap().settled_at;
    let mut actual = Vec::new();
    for (j, record) in records[CHOSEN..].iter().enumerate() {
        let observed: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(attribute, _)| schema.attribute(attribute).role == AttributeRole::Observed)
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect();
        for s in resolver.answer(&schema, &record.id, &observed, &BTreeMap::new()) {
            actual.push(decision("decision", &record.id, &s));
        }

        if let Some(first) = record.sets.get("labels").and_then(|l| l.first()) {
            let sets = BTreeMap::from([("labels".to_owned(), vec![first.clone()])]);
            let s = resolver.answer_one(&schema, &record.id, "labels", &observed, &sets, None);
            let mut line = decision("set_decision", &record.id, &s);
            line["chosen"] = json!(first);
            actual.push(line);
        }

        for length in [1, 2] {
            let typed = &record.values["owner"][..length];
            let s = resolver.answer_one(
                &schema,
                &record.id,
                "owner",
                &observed,
                &BTreeMap::new(),
                Some(typed),
            );
            let mut line = decision("typed_decision", &record.id, &s);
            line["typed"] = json!(typed);
            actual.push(line);
        }

        resolver.rebuild(&schema, std::slice::from_ref(record));
        if j % 4 == 0 {
            for s in resolver.answer(&schema, &record.id, &observed, &BTreeMap::new()) {
                actual.push(decision("saved_decision", &record.id, &s));
            }
        }

        if j % 6 == 5 {
            let mut again = early[j].clone();
            let at = TOPICS
                .iter()
                .position(|t| *t == again.values["topic"])
                .map_or(0, |i| i + 1);
            again.values.insert(
                "topic".to_owned(),
                TOPICS[at % (TOPICS.len() - 1)].to_owned(),
            );
            latest += minute;
            again.settled_at = latest;
            resolver.rebuild(&schema, &[again]);
        }

        if j % 10 == 9 {
            let mut copy = early[j * 2].clone();
            copy.id = format!("{}-copy", copy.id);
            latest += minute;
            copy.settled_at = latest;
            resolver.rebuild(&schema, &[copy]);
        }
    }

    assert_eq!(actual.len(), expected.len());
    for (at, (actual, expected)) in actual.into_iter().zip(expected).enumerate() {
        assert_eq!(
            doubles(actual),
            doubles(expected),
            "decision line {}",
            at + 1
        );
    }
}

/// The typed replay's decision logs (`note-logs-out.jsonl`, written by the .NET implementation): for the owner's
/// typed key thresholds on the first notes, every note asked about in settlement order with the key layer's best value
/// with none, one and two characters of its owner typed — score, value, whether it is the owner — to the bit.
#[test]
fn the_typed_replay_writes_the_golden_decision_records() {
    let records = notes();
    let golden = fixture("note-golden.jsonl");
    let schema = note_form(&golden, |l| l["kind"] == "typed" && l["field"] == "owner");
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&common::COUNTED[..]),
    };
    let mut logs = Vec::new();
    let typed = gil::replay::select_typed_key_thresholds_with(
        &mut KeyMemory::default(),
        &schema,
        "owner",
        &records[..CHOSEN],
        choice,
        2,
        &mut |r| logs.push(r.clone()),
    );
    assert_eq!(
        typed.thresholds(),
        select_typed_key_thresholds(
            &mut KeyMemory::default(),
            &schema,
            "owner",
            &records[..CHOSEN],
            choice,
            2,
        )
        .thresholds()
    );

    let expected: Vec<Value> = lines(&fixture("note-records-out.jsonl")).collect();
    assert_eq!(logs.len(), expected.len());
    for (at, (r, expected)) in logs.iter().zip(expected).enumerate() {
        let actual = json!({
            "kind": "typed_record",
            "field": r.attribute,
            "record": r.record,
            "order": r.order,
            "weight": r.weight,
            "typed": r.typed.as_ref().expect("a typed replay's record").iter().map(|t| t.as_ref().map(|t| json!([t.strength, t.value, t.correct]))).collect::<Vec<_>>(),
            "settled": r.settled,
        });
        assert_eq!(
            doubles(actual),
            doubles(expected),
            "typed record line {}",
            at + 1
        );
    }
}
