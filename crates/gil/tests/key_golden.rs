//! The key layer golden (`tests/fixtures/core/keys.jsonl`), written by the .NET implementation: the settled attribute
//! memory's ranking for each single-valued decided attribute of every log after the first 200, with the settings the
//! golden chose (`golden.jsonl`), must come out the same here — values, scores to the bit, evidence and trust.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, fixture, lines, schema, ticks, to_record};
use gil::key_memory::{KeyMemory, Lookup};
use gil::schema::AttributeRole;
use serde_json::{Value, json};

#[test]
fn the_key_layer_reproduces_its_golden() {
    let logs: Vec<Value> = lines(&fixture("records.jsonl")).collect();
    let schema = schema(&fixture("golden.jsonl"), |_| false);
    let expected: Vec<Value> = lines(&fixture("keys.jsonl")).collect();

    let mut memory = KeyMemory::default();
    for log in &logs[..CHOSEN] {
        memory.put(&schema, &to_record(log).remembered());
    }

    let mut at = 0;
    for log in &logs[CHOSEN..] {
        let observed: BTreeMap<String, String> = log["values"]
            .as_object()
            .expect("values")
            .iter()
            .filter(|(attribute, _)| schema.attribute(attribute).role == AttributeRole::Observed)
            .map(|(attribute, value)| {
                (
                    attribute.clone(),
                    value.as_str().expect("a value").to_owned(),
                )
            })
            .collect();
        for attribute in ["team", "code"] {
            let candidates: Vec<Value> = memory
                .rank(&Lookup::new(&schema, attribute, &observed), 3)
                .into_iter()
                .map(|c| {
                    json!([
                        c.value,
                        c.score,
                        c.evidence.as_ref().map(ToString::to_string),
                        c.trusted
                    ])
                })
                .collect();
            let actual = json!({
                "kind": "key_rank",
                "doc": log["id"],
                "field": attribute,
                "candidates": candidates,
            });
            assert_eq!(actual, expected[at], "line {}", at + 1);
            at += 1;
        }

        memory.put(&schema, &to_record(log).remembered());
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
