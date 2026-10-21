//! The key threshold golden (`tests/fixtures/core/replay.jsonl`): the key layer's
//! threshold replay for each single-valued decided attribute on the first 200 logs, with the schema as the golden had
//! configured it when that attribute's layers were chosen, must come out the same here — every count, every number to the
//! bit, and the state under each key — the golden's own replays of the attribute that takes several values and of both layers
//! of each single-valued attribute, and those layers again with records settled twice (`layers.jsonl`).

mod common;

use common::{CHOSEN, COUNTED, Golden, doubles, fixture, lines, replay, schema, to_record};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, ThresholdRule, select_coarse, select_key_threshold, select_layers,
    select_layers_by_rule,
};
use gil::schema::Record;
use serde_json::{Value, json};

#[test]
fn the_key_threshold_replay_reproduces_its_golden() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let mut golden_out = Golden::open("replay.jsonl");
    for attribute in ["team", "code"] {
        // As configured when the attribute's layers were chosen: every setting chosen before its `layers` line.
        let schema = schema(&golden, |l| {
            l["kind"] == "layers" && l["field"] == attribute
        });
        let replay = select_key_threshold(
            &mut KeyMemory::default(),
            &schema,
            attribute,
            &early,
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
        );
        let actual =
            json!({ "kind": "key_threshold", "field": attribute, "replay": self::replay(&replay) });
        golden_out.check(actual, attribute);
    }
    golden_out.finish();
}

/// The golden's own line for the attribute that takes several values (`golden.jsonl`, kind `key`): chosen after every other
/// attribute's layers, counting every settlement.
#[test]
fn the_set_field_replay_reproduces_its_golden() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let expected = lines(&golden)
        .find(|l| l["kind"] == "key" && l["field"] == "tags")
        .expect("the set field's line");

    let schema = schema(&golden, |l| l["kind"] == "key" && l["field"] == "tags");
    let replay = select_key_threshold(
        &mut KeyMemory::default(),
        &schema,
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

/// The golden's own lines for both layers of each single-valued decided attribute (`golden.jsonl`, kind `layers`): the key
/// replay and the similar record replay on what the chosen key threshold would not have answered, with its
/// similarity floor — the schema as configured when that attribute's layers were chosen.
#[test]
fn the_layers_replay_reproduces_its_golden() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    for attribute in ["team", "code"] {
        let is_layers = |l: &Value| l["kind"] == "layers" && l["field"] == attribute;
        let expected = lines(&golden)
            .find(is_layers)
            .expect("the field's layers line");
        let schema = schema(&golden, is_layers);
        let layers = select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &early,
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
        );
        let actual = json!({
            "kind": "layers",
            "field": attribute,
            "key": replay(&layers.key),
            "memory": replay(&layers.memory),
        });
        assert_eq!(doubles(actual), doubles(expected), "attribute {attribute}");
    }
}

/// The golden's own line for the coarse level of the attribute that has one (`golden.jsonl`, kind `coarse`): the class the
/// keys back on the lookups the attribute's value layers leave at their chosen thresholds, then the class the similar
/// records vote for on what that leaves — the schema as configured when the level was chosen.
#[test]
fn the_coarse_replay_reproduces_its_golden() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let is_coarse = |l: &Value| l["kind"] == "coarse" && l["field"] == "code";
    let expected = lines(&golden).find(is_coarse).expect("the coarse line");
    let schema = schema(&golden, is_coarse);
    let coarse = select_coarse(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
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

/// The finer layers golden (`tests/fixtures/core/layers.jsonl`): both layers replayed with the same schemas on the first
/// logs followed by twenty of them settled again under their own ids and twenty copied under new ids, each later —
/// a record passed over as its own evidence, and a case of the same evidence keeping only its latest record.
#[test]
fn the_layers_replay_with_documents_settled_again_reproduces_its_golden() {
    let early: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let last = early[CHOSEN - 1].settled_at;
    let mut again = early.clone();
    for (i, record) in early.iter().take(40).enumerate() {
        let mut copy = record.clone();
        if i % 2 == 1 {
            copy.id = format!("{}-copy", record.id);
        }
        copy.settled_at = last + (i as i64 + 1) * 60 * 10_000_000;
        again.push(copy);
    }

    let golden = fixture("golden.jsonl");
    let mut golden_out = Golden::open("layers.jsonl");
    for attribute in ["team", "code"] {
        let schema = schema(&golden, |l| {
            l["kind"] == "layers" && l["field"] == attribute
        });
        let layers = select_layers(
            &mut KeyMemory::default(),
            &mut LexicalMemory::default(),
            &schema,
            attribute,
            &again,
            Choice {
                target_precision: 0.8,
                minimum_answered: 10,
                counting: Some(&COUNTED),
            },
        );
        let actual = json!({
            "kind": "layers_again",
            "field": attribute,
            "key": replay(&layers.key),
            "memory": replay(&layers.memory),
        });
        golden_out.check(actual, attribute);
    }
    golden_out.finish();
}

/// The pooled golden (`tests/fixtures/core/pooled.jsonl`): [`ThresholdRule::Pooled`] on the records laid end to end
/// fifteen times, each copy under its own ids and later than the one before — with the schema as the golden had it when
/// each attribute's layers were chosen, and again with that attribute's chosen thresholds — must come out the same here.
#[test]
fn the_pooled_rule_reproduces_its_golden() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let span = records[records.len() - 1].settled_at - records[0].settled_at + 60 * 10_000_000;
    let mut laid = Vec::with_capacity(records.len() * 15);
    for k in 0..15_i64 {
        for record in &records {
            let mut copy = record.clone();
            if k > 0 {
                copy.id = format!("{}-r{k}", record.id);
                copy.settled_at = record.settled_at + span * k;
            }
            laid.push(copy);
        }
    }

    let golden = fixture("golden.jsonl");
    let mut golden_out = Golden::open("pooled.jsonl");
    for attribute in ["team", "code"] {
        let standing = schema(&golden, |l| {
            l["kind"] == "layers" && l["field"] == attribute
        });
        let layers_line = lines(&golden)
            .find(|l| l["kind"] == "layers" && l["field"] == attribute)
            .expect("the attribute's layers line");
        let mut chosen = standing.clone();
        let definition = chosen
            .attributes
            .iter_mut()
            .find(|a| a.name == attribute)
            .expect("the attribute");
        definition.key_threshold = layers_line["key"]["chosen"]["threshold"].as_f64();
        definition.memory_threshold = layers_line["memory"]["chosen"]["threshold"].as_f64();
        definition.memory_floor = layers_line["memory"]["chosen"]["similarity_floor"].as_f64();

        for (as_it_stands, schema) in [("none", &standing), ("chosen", &chosen)] {
            let layers = select_layers_by_rule(
                &mut KeyMemory::default(),
                &mut LexicalMemory::default(),
                schema,
                attribute,
                &laid,
                Choice {
                    target_precision: 0.8,
                    minimum_answered: 10,
                    counting: Some(&COUNTED),
                },
                ThresholdRule::Pooled,
            );
            let actual = json!({
                "kind": "layers_pooled",
                "field": attribute,
                "as_it_stands": as_it_stands,
                "key": replay(&layers.key),
                "memory": replay(&layers.memory),
            });
            golden_out.check(
                actual,
                format!("{attribute}, thresholds as they stand: {as_it_stands}"),
            );
        }
    }
    golden_out.finish();
}
