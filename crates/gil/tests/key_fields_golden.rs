//! The key attribute and concentration goldens: the schema path's choice for its team
//! (`golden.jsonl`, kinds `key_fields` and `key_concentrations`) and, with every trial, that choice again and the
//! choice on made-up inspections (`key_fields.jsonl` on `inspection.jsonl`) — where a broad attribute alone answers more
//! often than the attribute naming what the decided attribute describes, and the gain over the target keeps the latter — must come
//! out the same here, every number to the bit.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, COUNTED, Golden, fixture, lines, replay, schema, to_record};
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
    Schema::new(
        "inspection",
        vec![
            Attribute::new("outlet", AttributeRole::Observed),
            Attribute::new("kind", AttributeRole::Observed),
            Attribute::new("inspector", AttributeRole::Observed),
            risk,
        ],
    )
    .expect("a valid schema")
}

#[test]
fn the_form_paths_key_fields_and_concentrations_reproduce_its_golden() {
    let early = early();
    let golden = fixture("golden.jsonl");
    let schema = schema(&golden, |_| true); // the schema as declared
    let mut golden_out = Golden::part("golden.jsonl", &["key_fields", "key_concentrations"]);

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
    golden_out.check(actual, "key_fields team");

    let mut attributes = schema.attributes().to_vec();
    for f in attributes.iter_mut().filter(|f| f.name == "team") {
        f.key_fields = chosen.key_fields.clone();
    }
    let schema = Schema::new(schema.name(), attributes).expect("a valid schema");
    let found = select_key_concentrations(&mut KeyMemory::default(), &schema, "team", &early);
    let actual = json!({
        "kind": "key_concentrations",
        "field": "team",
        "concentrations": concentrations(&found),
    });
    golden_out.check(actual, "key_concentrations team");
    golden_out.finish();
}

#[test]
fn key_fields_with_every_trial_reproduce_their_golden() {
    let mut golden = Golden::open("key_fields.jsonl");

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
    golden.check(choice("ticket", "team", &ticket), "ticket");

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
    golden.check(actual, "inspection concentrations");

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
    golden.check(choice("inspection", "risk", &chosen), "inspection");

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
    golden.check(choice("ticket-typed", "team", &typed), "ticket-typed");
    golden.finish();
}
