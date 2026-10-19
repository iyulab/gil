//! The decision record golden (`tests/fixtures/core/records-out.jsonl`), written by the .NET implementation from the
//! replay its layers threshold choice rests on: for the layers replay with documents settled again (`layers.jsonl`), every
//! document asked about, in settlement order, with what the key layer and the similar document vote made of it whatever
//! the thresholds and what it was settled as — the same here, every number to the bit. The reference replay does not
//! keep the best value itself; where it says the best value was right, the value here must be the settled one.

mod common;

use common::{CHOSEN, COUNTED, doubles, fixture, form, lines};
use gilfit::decision_record::DecisionRecord;
use gilfit::field_memory::FieldMemory;
use gilfit::form::Document;
use gilfit::lexical_memory::LexicalMemory;
use gilfit::threshold_selection::{Choice, select_layers, select_layers_with};
use serde_json::{Value, json};

fn line(r: &DecisionRecord) -> Value {
    json!({
        "kind": "record",
        "field": r.field,
        "record": r.record,
        "order": r.order,
        "weight": r.weight,
        "key": r.key.as_ref().map(|k| json!([k.strength, k.correct, k.key, k.caught])),
        "similar": r.similar.as_ref().map(|s| json!([s.score, s.correct, s.nearest, s.voted])),
        "settled": r.settled,
        "settlement": r.settlement.map(|k| format!("{k:?}")),
        "probability": r.probability,
    })
}

#[test]
fn the_layers_replay_writes_the_golden_decision_records() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| common::document(&r))
        .collect();
    let last = early[CHOSEN - 1].settled_at;
    let mut again = early.clone();
    for (i, document) in early.iter().take(40).enumerate() {
        let mut copy = document.clone();
        if i % 2 == 1 {
            copy.id = format!("{}-copy", document.id);
        }
        copy.settled_at = last + (i as i64 + 1) * 60 * 10_000_000;
        again.push(copy);
    }

    let golden = fixture("golden.jsonl");
    let expected: Vec<Value> = lines(&fixture("records-out.jsonl")).collect();
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    let mut at = 0;
    for field in ["team", "code"] {
        let form = form(&golden, |l| l["kind"] == "layers" && l["field"] == field);
        let mut records = Vec::new();
        let layers = select_layers_with(
            &mut FieldMemory::default(),
            &mut LexicalMemory::default(),
            &form,
            field,
            &again,
            choice,
            &mut |r| records.push(r.clone()),
        );
        // Writing records changes nothing the choice comes to.
        assert_eq!(
            layers,
            select_layers(
                &mut FieldMemory::default(),
                &mut LexicalMemory::default(),
                &form,
                field,
                &again,
                choice,
            ),
            "field {field}"
        );

        for record in &records {
            assert_eq!(
                doubles(line(record)),
                doubles(expected[at].clone()),
                "record line {}",
                at + 1
            );
            let settled = record.settled.as_deref();
            if let Some(key) = &record.key {
                assert_eq!(
                    key.correct == Some(true),
                    Some(key.value.as_str()) == settled
                );
            }
            if let Some(similar) = &record.similar {
                assert_eq!(
                    similar.correct == Some(true),
                    Some(similar.value.as_str()) == settled
                );
            }
            at += 1;
        }
    }

    assert_eq!(at, expected.len());
}
