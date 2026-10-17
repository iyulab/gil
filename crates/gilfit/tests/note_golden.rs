//! The note golden (`tests/fixtures/core/note-golden.jsonl`), written by the .NET implementation on the made-up notes of
//! `note-records.jsonl`: a form whose long free text decides a judged field through the similar document layer, with a
//! value that must not be missed, a value settled outside the field's candidates, a coarse level made from a table, a
//! field that takes several values, values arriving in different orders, and documents settled again and copied while
//! the resolver is asked. The settings it chose and its decisions — before typing, with a set field's first value
//! chosen, and with the owner's first characters typed — must come out the same here, to the bit.

mod common;

use std::collections::BTreeMap;

use common::{document, doubles, fixture, lines, replay};
use gilfit::field_memory::FieldMemory;
use gilfit::form::{CoarseLevel, Document, Field, Form, Role};
use gilfit::form_resolver::{Candidate, FormResolver, Suggestion};
use gilfit::lexical_memory::LexicalMemory;
use gilfit::threshold_selection::{
    select_coarse, select_key_threshold, select_layers, select_typed_key_thresholds,
};
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

/// The note form with the settings the golden chose before the first line `stop` holds for.
fn note_form(golden: &str, stop: impl Fn(&Value) -> bool) -> Form {
    let mut topic = Field::new("topic", Role::Judged);
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
    let mut labels = Field::new("labels", Role::Judged);
    labels.multiple = true;
    labels.depends_on = Some(vec!["desk".to_owned()]);
    let mut owner = Field::new("owner", Role::Judged);
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
                target.memory_similarity_floor = chosen(&line["memory"], "similarity_floor");
            }
            Some("coarse") => {
                let level = target.coarse.as_mut().expect("a coarse level");
                level.key_threshold = chosen(&line["key"], "threshold");
                level.memory_threshold = chosen(&line["memory"], "threshold");
                level.memory_similarity_floor = chosen(&line["memory"], "similarity_floor");
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

    Form {
        name: "note".to_owned(),
        fields: vec![
            Field::new("desk", Role::Observed),
            Field::new("body", Role::Observed),
            Field::new("ref", Role::Observed),
            topic,
            labels,
            owner,
        ],
    }
}

fn notes() -> Vec<Document> {
    lines(&fixture("note-records.jsonl"))
        .map(|r| document(&r))
        .collect()
}

#[test]
fn the_note_settings_reproduce_their_golden() {
    let documents = notes();
    let early = &documents[..CHOSEN];
    let golden = fixture("note-golden.jsonl");
    let counted = Some(&common::COUNTED[..]);
    for expected in lines(&golden)
        .filter(|l| ["layers", "coarse", "key", "typed"].contains(&l["kind"].as_str().unwrap()))
    {
        let (kind, field) = (
            expected["kind"].as_str().unwrap().to_owned(),
            expected["field"].as_str().unwrap().to_owned(),
        );
        let (kind, field) = (kind.as_str(), field.as_str());
        let form = note_form(&golden, |l| l["kind"] == kind && l["field"] == field);
        let actual = match kind {
            "layers" | "coarse" => {
                let choose = if kind == "layers" {
                    select_layers
                } else {
                    select_coarse
                };
                let layers = choose(
                    &mut FieldMemory::default(),
                    &mut LexicalMemory::default(),
                    &form,
                    field,
                    early,
                    0.8,
                    10,
                    counted,
                );
                json!({ "kind": kind, "field": field, "key": replay(&layers.key), "memory": replay(&layers.memory) })
            }
            "typed" => {
                let typed = select_typed_key_thresholds(
                    &mut FieldMemory::default(),
                    &form,
                    field,
                    early,
                    0.8,
                    10,
                    2,
                    counted,
                );
                json!({ "kind": kind, "field": field, "replays": typed.by_length.iter().map(replay).collect::<Vec<_>>() })
            }
            _ => {
                let replay = select_key_threshold(
                    &mut FieldMemory::default(),
                    &form,
                    field,
                    early,
                    0.8,
                    10,
                    None,
                );
                json!({ "kind": kind, "field": field, "replay": self::replay(&replay) })
            }
        };
        assert_eq!(doubles(actual), doubles(expected), "{kind} {field}");
    }
}

fn candidate(c: &Candidate) -> Value {
    json!([c.value, c.score, c.source.name(), c.evidence, c.trusted])
}

fn decision(kind: &str, id: &str, s: &Suggestion) -> Value {
    json!({
        "kind": kind,
        "doc": id,
        "field": s.field,
        "answered": s.answered(),
        "source": s.source.name(),
        "candidates": s.candidates.iter().map(candidate).collect::<Vec<_>>(),
        "coarse": s.coarse.as_ref().map(candidate),
    })
}

#[test]
fn the_note_decisions_reproduce_their_golden() {
    let documents = notes();
    let early = &documents[..CHOSEN];
    let golden = fixture("note-golden.jsonl");
    let form = note_form(&golden, |_| false);
    let expected: Vec<Value> = lines(&golden)
        .filter(|l| l["kind"].as_str().unwrap().ends_with("decision"))
        .collect();

    let mut resolver = FormResolver::new(FieldMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&form, early);
    let minute = 60 * 10_000_000;
    let mut latest = documents.last().unwrap().settled_at;
    let mut actual = Vec::new();
    for (j, document) in documents[CHOSEN..].iter().enumerate() {
        let observed: BTreeMap<String, String> = document
            .values
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        for s in resolver.suggest(&form, &document.id, &observed, &BTreeMap::new()) {
            actual.push(decision("decision", &document.id, &s));
        }

        if let Some(first) = document.sets.get("labels").and_then(|l| l.first()) {
            let sets = BTreeMap::from([("labels".to_owned(), vec![first.clone()])]);
            let s = resolver.suggest_one(&form, &document.id, "labels", &observed, &sets, None);
            let mut line = decision("set_decision", &document.id, &s);
            line["chosen"] = json!(first);
            actual.push(line);
        }

        for length in [1, 2] {
            let typed = &document.values["owner"][..length];
            let s = resolver.suggest_one(
                &form,
                &document.id,
                "owner",
                &observed,
                &BTreeMap::new(),
                Some(typed),
            );
            let mut line = decision("typed_decision", &document.id, &s);
            line["typed"] = json!(typed);
            actual.push(line);
        }

        resolver.rebuild(&form, std::slice::from_ref(document));
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
            resolver.rebuild(&form, &[again]);
        }

        if j % 10 == 9 {
            let mut copy = early[j * 2].clone();
            copy.id = format!("{}-copy", copy.id);
            latest += minute;
            copy.settled_at = latest;
            resolver.rebuild(&form, &[copy]);
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
