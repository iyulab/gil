//! The key threshold golden (`tests/fixtures/core/replay.jsonl`), written by the .NET implementation: the key layer's
//! threshold replay for each single-valued judged field on the first 200 records, with the form as the golden had
//! configured it when that field's layers were chosen, must come out the same here — every count, every number to the
//! bit, and the state under each key — and the golden's own replay of the field that takes several values.

mod common;

use common::{CHOSEN, document, fixture, form, lines};
use gilfit::field_memory::FieldMemory;
use gilfit::form::{Document, SettlementKind};
use gilfit::threshold_selection::{ThresholdChoice, ThresholdReplay, select_key_threshold};
use serde_json::{Number, Value, json};

const COUNTED: [SettlementKind; 4] = [
    SettlementKind::Enter,
    SettlementKind::Accept,
    SettlementKind::Correct,
    SettlementKind::Check,
];

/// The replay in the golden's shape.
fn replay(replay: &ThresholdReplay) -> Value {
    json!({
        "lookups": replay.lookups,
        "candidates": replay.candidates,
        "chosen": replay.chosen.as_ref().map(choice),
        "most_precise": replay.most_precise.as_ref().map(choice),
        "keys": replay.keys.as_ref().map(|keys| keys
            .iter()
            .map(|k| json!([k.key, k.lookups, k.answered, k.precision, k.answering]))
            .collect::<Vec<_>>()),
    })
}

fn choice(choice: &ThresholdChoice) -> Value {
    json!({
        "threshold": choice.threshold,
        "precision": choice.precision,
        "answered": choice.answered,
        "lookups": choice.lookups,
        "answered_weight": choice.answered_weight,
        "effective_answered": choice.effective_answered,
        "similarity_floor": choice.similarity_floor,
        "promise": choice.promise,
        "promise_answered": choice.promise_answered,
    })
}

/// Every number as the double it denotes: the reference writes a whole double without a fraction (`160`), which reads
/// back as an integer, so both sides compare as doubles — exactly.
fn doubles(value: Value) -> Value {
    match value {
        Value::Number(n) => {
            Value::Number(Number::from_f64(n.as_f64().expect("a number")).expect("a finite number"))
        }
        Value::Array(items) => Value::Array(items.into_iter().map(doubles).collect()),
        Value::Object(fields) => {
            Value::Object(fields.into_iter().map(|(k, v)| (k, doubles(v))).collect())
        }
        other => other,
    }
}

#[test]
fn the_key_threshold_replay_reproduces_its_golden() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let expected: Vec<Value> = lines(&fixture("replay.jsonl")).collect();

    let fields = ["team", "code"];
    assert_eq!(expected.len(), fields.len());
    for (field, expected) in fields.into_iter().zip(expected) {
        // As configured when the field's layers were chosen: every setting chosen before its `layers` line.
        let form = form(&golden, |l| l["kind"] == "layers" && l["field"] == field);
        let replay = select_key_threshold(
            &mut FieldMemory::default(),
            &form,
            field,
            &early,
            0.8,
            10,
            Some(&COUNTED),
        );
        let actual =
            json!({ "kind": "key_threshold", "field": field, "replay": self::replay(&replay) });
        assert_eq!(doubles(actual), doubles(expected), "field {field}");
    }
}

/// The golden's own line for the field that takes several values (`golden.jsonl`, kind `key`): chosen after every other
/// field's layers, counting every settlement.
#[test]
fn the_set_field_replay_reproduces_its_golden() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let expected = lines(&golden)
        .find(|l| l["kind"] == "key" && l["field"] == "tags")
        .expect("the set field's line");

    let form = form(&golden, |l| l["kind"] == "key" && l["field"] == "tags");
    let replay = select_key_threshold(
        &mut FieldMemory::default(),
        &form,
        "tags",
        &early,
        0.8,
        10,
        None,
    );
    let actual = json!({ "kind": "key", "field": "tags", "replay": self::replay(&replay) });
    assert_eq!(doubles(actual), doubles(expected));
}
