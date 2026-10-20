//! The key attribute and concentration goldens, written by the .NET implementation: the schema path's choice for its team
//! (`golden.jsonl`, kinds `key_fields` and `key_concentrations`) and, with every trial, that choice again and the
//! choice on made-up inspections (`key_fields.jsonl` on `inspection.jsonl`) — where a broad attribute alone answers more
//! often than the attribute naming what the decided attribute describes, and the gain over the target keeps the latter — must come
//! out the same here, every number to the bit.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, COUNTED, doubles, fixture, lines, replay, schema, to_record};
use gil::key_memory::KeyMemory;
use gil::replay::{
    Choice, KeyFieldsChoice, TypedKeyThresholds, select_key_concentrations, select_key_fields,
};
use gil::schema::{Attribute, AttributeRole, Record, Schema};
use serde_json::{Map, Value, json};

fn early() -> Vec<Record> {
    lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect()
}

fn strings(values: &Option<Vec<String>>) -> Value {
    json!(values)
}

fn concentrations(values: &BTreeMap<String, f64>) -> Value {
    let map: Map<String, Value> = values.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    Value::Object(map)
}

/// The line with the typed replays added where typing was counted, as the reference writes them.
fn with_typed(mut line: Value, typed: &Option<TypedKeyThresholds>) -> Value {
    if let Some(typed) = typed {
        line["typed"] = json!(typed.by_length.iter().map(replay).collect::<Vec<_>>());
    }
    line
}

fn choice(world: &str, attribute: &str, choice: &KeyFieldsChoice) -> Value {
    let trials: Vec<Value> = choice
        .trials
        .iter()
        .map(|t| {
            with_typed(
                json!({ "fields": t.key_fields, "replay": replay(&t.replay) }),
                &t.typed,
            )
        })
        .collect();
    with_typed(
        json!({
            "kind": "key_fields",
            "world": world,
            "field": attribute,
            "fields": strings(&choice.key_fields),
            "chosen": replay(&choice.chosen),
            "all_fields": replay(&choice.all_fields),
            "trials": trials,
        }),
        &choice.typed,
    )
}

fn inspection(concentrations: BTreeMap<String, f64>) -> Schema {
    let mut risk = Attribute::new("risk", AttributeRole::Decided);
    risk.key_concentrations = concentrations;
    Schema {
        name: "inspection".to_owned(),
        attributes: vec![
            Attribute::new("outlet", AttributeRole::Observed),
            Attribute::new("kind", AttributeRole::Observed),
            Attribute::new("inspector", AttributeRole::Observed),
            risk,
        ],
    }
}

#[test]
fn the_form_paths_key_fields_and_concentrations_reproduce_its_golden() {
    let early = early();
    let golden = fixture("golden.jsonl");
    let mut schema = schema(&golden, |_| true); // the schema as declared
    let expected: Vec<Value> = lines(&golden).take(2).collect();

    let chosen = select_key_fields(
        &schema,
        "team",
        &early,
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: Some(&COUNTED),
        },
        0,
    );
    let actual = json!({
        "kind": "key_fields",
        "field": "team",
        "fields": strings(&chosen.key_fields),
        "chosen": replay(&chosen.chosen),
    });
    assert_eq!(doubles(actual), doubles(expected[0].clone()));

    for f in schema.attributes.iter_mut().filter(|f| f.name == "team") {
        f.key_fields = chosen.key_fields.clone();
    }
    let found = select_key_concentrations(&mut KeyMemory::default(), &schema, "team", &early);
    let actual = json!({
        "kind": "key_concentrations",
        "field": "team",
        "concentrations": concentrations(&found),
    });
    assert_eq!(doubles(actual), doubles(expected[1].clone()));
}

#[test]
fn key_fields_with_every_trial_reproduce_their_golden() {
    let expected: Vec<Value> = lines(&fixture("key_fields.jsonl")).collect();
    assert_eq!(expected.len(), 4);

    let ticket = select_key_fields(
        &schema(&fixture("golden.jsonl"), |_| true),
        "team",
        &early(),
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: Some(&COUNTED),
        },
        0,
    );
    assert_eq!(
        doubles(choice("ticket", "team", &ticket)),
        doubles(expected[0].clone())
    );

    let inspections: Vec<Record> = lines(&fixture("inspection.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let found = select_key_concentrations(
        &mut KeyMemory::default(),
        &inspection(Default::default()),
        "risk",
        &inspections,
    );
    let actual = json!({
        "kind": "key_concentrations",
        "world": "inspection",
        "field": "risk",
        "concentrations": concentrations(&found),
    });
    assert_eq!(doubles(actual), doubles(expected[1].clone()));

    let chosen = select_key_fields(
        &inspection(found),
        "risk",
        &inspections,
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: None,
        },
        0,
    );
    assert_eq!(
        doubles(choice("inspection", "risk", &chosen)),
        doubles(expected[2].clone())
    );

    // The ticket's team again, counting the answers while its first two characters are typed as well.
    let typed = select_key_fields(
        &schema(&fixture("golden.jsonl"), |_| true),
        "team",
        &early(),
        Choice {
            target_precision: 0.8,
            minimum_answered: 10,
            counting: Some(&COUNTED),
        },
        2,
    );
    assert_eq!(
        doubles(choice("ticket-typed", "team", &typed)),
        doubles(expected[3].clone())
    );
}
