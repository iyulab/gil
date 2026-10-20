//! The key threshold golden (`tests/fixtures/core/replay.jsonl`), written by the .NET implementation: the key layer's
//! threshold replay for each single-valued judged field on the first 200 records, with the form as the golden had
//! configured it when that field's layers were chosen, must come out the same here — every count, every number to the
//! bit, and the state under each key — the golden's own replays of the field that takes several values and of both layers
//! of each single-valued field, and those layers again with documents settled twice (`layers.jsonl`).

mod common;

use common::{CHOSEN, COUNTED, document, doubles, fixture, form, lines, replay};
use gil::field_memory::FieldMemory;
use gil::form::Document;
use gil::lexical_memory::LexicalMemory;
use gil::threshold_selection::{Choice, select_coarse, select_key_threshold, select_layers};
use serde_json::{Value, json};

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
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
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
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: None,
        },
    );
    let actual = json!({ "kind": "key", "field": "tags", "replay": self::replay(&replay) });
    assert_eq!(doubles(actual), doubles(expected));
}

/// The golden's own lines for both layers of each single-valued judged field (`golden.jsonl`, kind `layers`): the key
/// replay and the similar document replay on what the chosen key threshold would not have answered, with its
/// similarity floor — the form as configured when that field's layers were chosen.
#[test]
fn the_layers_replay_reproduces_its_golden() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    for field in ["team", "code"] {
        let is_layers = |l: &Value| l["kind"] == "layers" && l["field"] == field;
        let expected = lines(&golden)
            .find(is_layers)
            .expect("the field's layers line");
        let form = form(&golden, is_layers);
        let layers = select_layers(
            &mut FieldMemory::default(),
            &mut LexicalMemory::default(),
            &form,
            field,
            &early,
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
        );
        let actual = json!({
            "kind": "layers",
            "field": field,
            "key": replay(&layers.key),
            "memory": replay(&layers.memory),
        });
        assert_eq!(doubles(actual), doubles(expected), "field {field}");
    }
}

/// The golden's own line for the coarse level of the field that has one (`golden.jsonl`, kind `coarse`): the class the
/// keys back on the lookups the field's value layers leave at their chosen thresholds, then the class the similar
/// documents vote for on what that leaves — the form as configured when the level was chosen.
#[test]
fn the_coarse_replay_reproduces_its_golden() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let is_coarse = |l: &Value| l["kind"] == "coarse" && l["field"] == "code";
    let expected = lines(&golden).find(is_coarse).expect("the coarse line");
    let form = form(&golden, is_coarse);
    let coarse = select_coarse(
        &mut FieldMemory::default(),
        &mut LexicalMemory::default(),
        &form,
        "code",
        &early,
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: Some(&COUNTED),
        },
    );
    let actual = json!({
        "kind": "coarse",
        "field": "code",
        "key": replay(&coarse.key),
        "memory": replay(&coarse.memory),
    });
    assert_eq!(doubles(actual), doubles(expected));
}

/// The finer layers golden (`tests/fixtures/core/layers.jsonl`): both layers replayed with the same forms on the first
/// records followed by twenty of them settled again under their own ids and twenty copied under new ids, each later —
/// a document passed over as its own evidence, and a case of the same evidence keeping only its latest document.
#[test]
fn the_layers_replay_with_documents_settled_again_reproduces_its_golden() {
    let early: Vec<Document> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| document(&r))
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
    let expected: Vec<Value> = lines(&fixture("layers.jsonl")).collect();
    assert_eq!(expected.len(), 2);
    for (field, expected) in ["team", "code"].into_iter().zip(expected) {
        let form = form(&golden, |l| l["kind"] == "layers" && l["field"] == field);
        let layers = select_layers(
            &mut FieldMemory::default(),
            &mut LexicalMemory::default(),
            &form,
            field,
            &again,
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
        );
        let actual = json!({
            "kind": "layers_again",
            "field": field,
            "key": replay(&layers.key),
            "memory": replay(&layers.memory),
        });
        assert_eq!(doubles(actual), doubles(expected), "field {field}");
    }
}
