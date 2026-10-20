//! The similar record memory golden (`tests/fixtures/core/lexical.jsonl`), written by the .NET implementation: for each
//! single-valued decided attribute, the first 200 logs remembered by their evidence, then for every later log the 11
//! records most like its observed values — before it is remembered, with some records forgotten and some remembered
//! again along the way — must come out the same here: sources, similarities to the bit, answers and the order of ties.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil::lexical_memory::LexicalMemory;
use gil::schema::{AttributeRole, Record, Schema};
use serde_json::{Value, json};

/// How many similar records a lookup asks for: the resolver's default of ten voters, and one more for the record
/// itself.
const ASKED: usize = 11;

const FIELDS: [&str; 2] = ["team", "code"];

fn task(attribute: &str) -> String {
    format!("ticket/{attribute}")
}

/// Remembers the record for each attribute it holds a value of (once the values the application filled are left out),
/// and forgets it for the others.
fn put(memory: &mut LexicalMemory, schema: &Schema, settled: &Record) {
    let record = settled.remembered();
    for attribute in FIELDS {
        match record.values.get(attribute) {
            Some(value) => memory.remember(
                &task(attribute),
                &record.id,
                &schema.evidence(attribute, &record.values, &record.sets),
                value,
            ),
            None => memory.forget(&task(attribute), &record.id),
        }
    }
}

#[test]
fn the_similar_document_memory_reproduces_its_golden() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let schema = schema(&fixture("golden.jsonl"), |_| true); // the schema as declared: evidence needs no chosen setting
    let expected: Vec<Value> = lines(&fixture("lexical.jsonl")).collect();

    let mut memory = LexicalMemory::default();
    for record in &records[..CHOSEN] {
        put(&mut memory, &schema, record);
    }

    let mut at = 0;
    for i in CHOSEN..records.len() {
        let record = &records[i];
        let observed: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(attribute, _)| schema.attribute(attribute).role == AttributeRole::Observed)
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect();
        for attribute in FIELDS {
            let state = schema.evidence(attribute, &observed, &BTreeMap::new());
            let matches: Vec<Value> = memory
                .nearest(&task(attribute), &state, ASKED)
                .into_iter()
                .map(|m| json!([m.source, m.similarity, m.answer]))
                .collect();
            let actual = json!({
                "kind": "nearest",
                "doc": record.id,
                "field": attribute,
                "count": memory.count(&task(attribute)),
                "matches": matches,
            });
            assert_eq!(actual, expected[at], "line {}", at + 1);
            at += 1;
        }

        put(&mut memory, &schema, record);
        if i % 10 == 0 {
            for attribute in FIELDS {
                memory.forget(&task(attribute), &records[i - 150].id);
            }
        }
        if i % 7 == 0 {
            put(&mut memory, &schema, &records[i - 1]); // remembered again: keeps its place among ties
        }
    }

    assert_eq!(at, expected.len());
}

#[test]
fn the_same_text_is_a_cosine_of_one() {
    let mut memory = LexicalMemory::default();
    memory.remember("t", "a", "router lights blinking", "network");
    memory.remember("t", "b", "결제가 두 번 되었습니다", "billing");
    let found = memory.nearest("t", "Router  LIGHTS blinking", 2);
    assert_eq!(found[0].source, "a");
    assert!((found[0].similarity - 1.0).abs() < 1e-12);
    assert_eq!(found[1].similarity, 0.0);
}
