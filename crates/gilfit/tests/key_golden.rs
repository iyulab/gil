//! The key layer golden (`tests/fixtures/core/keys.jsonl`), written by the .NET implementation: the settled field
//! memory's ranking for each single-valued judged field of every record after the first 200, with the settings the
//! golden chose (`golden.jsonl`), must come out the same here — values, scores to the bit, evidence and trust.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, document, fixture, form, lines, ticks};
use gilfit::field_memory::{FieldMemory, Lookup};
use gilfit::form::Role;
use serde_json::{Value, json};

#[test]
fn the_key_layer_reproduces_its_golden() {
    let records: Vec<Value> = lines(&fixture("records.jsonl")).collect();
    let form = form(&fixture("golden.jsonl"), |_| false);
    let expected: Vec<Value> = lines(&fixture("keys.jsonl")).collect();

    let mut memory = FieldMemory::default();
    for record in &records[..CHOSEN] {
        memory.put(&form, &document(record).remembered());
    }

    let mut at = 0;
    for record in &records[CHOSEN..] {
        let observed: BTreeMap<String, String> = record["values"]
            .as_object()
            .expect("values")
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.as_str().expect("a value").to_owned()))
            .collect();
        for field in ["team", "code"] {
            let candidates: Vec<Value> = memory
                .rank(&Lookup::new(&form, field, &observed), 3)
                .into_iter()
                .map(|c| json!([c.value, c.score, c.evidence, c.trusted]))
                .collect();
            let actual = json!({
                "kind": "key_rank",
                "doc": record["id"],
                "field": field,
                "candidates": candidates,
            });
            assert_eq!(actual, expected[at], "line {}", at + 1);
            at += 1;
        }

        memory.put(&form, &document(record).remembered());
    }

    assert_eq!(at, expected.len());
}

#[test]
fn ticks_count_from_the_unix_epoch_in_utc() {
    assert_eq!(ticks("1970-01-01T00:00:00.0000000+00:00"), 0);
    assert_eq!(ticks("1970-01-01T09:00:00.0000000+09:00"), 0);
    assert_eq!(
        ticks("2026-01-01T00:07:00.0000001+00:00"),
        17_672_260_200_000_001
    );
}
