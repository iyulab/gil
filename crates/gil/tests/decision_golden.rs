//! The decision lines of the schema path's golden (`tests/fixtures/core/golden.jsonl`, kind `decision`), written by the
//! .NET implementation: a resolver rebuilt from the first 200 logs with the settings the golden chose, asked about
//! every decided attribute of each later log from its observed values before that log is put, must answer the same
//! here — whether it answered, the first candidate's layer, the first three candidates (values, scores to the bit,
//! layers, trust) and the coarse class.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, doubles, fixture, lines, schema, to_record};
use gil::decider::{Candidate, Decider};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::schema::{AttributeRole, Record};
use serde_json::{Value, json};

fn candidate(c: &Candidate) -> Value {
    json!([c.value, c.score, c.source.name(), c.trusted])
}

#[test]
fn the_resolver_reproduces_the_golden_decisions() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let schema = schema(&golden, |_| false);
    let expected: Vec<Value> = lines(&golden).filter(|l| l["kind"] == "decision").collect();

    let mut resolver = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&schema, &records[..CHOSEN]);
    let mut at = 0;
    for record in &records[CHOSEN..] {
        let observed: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(attribute, _)| schema.attribute(attribute).role == AttributeRole::Observed)
            .map(|(attribute, value)| (attribute.clone(), value.clone()))
            .collect();
        for suggestion in resolver.answer(&schema, &record.id, &observed, &BTreeMap::new()) {
            let actual = json!({
                "kind": "decision",
                "doc": record.id,
                "field": suggestion.attribute,
                "answered": suggestion.answered(),
                "source": suggestion.source.name(),
                "candidates": suggestion.candidates.iter().map(candidate).collect::<Vec<_>>(),
                "coarse": suggestion.coarse.as_ref().map(candidate),
            });
            assert_eq!(
                doubles(actual),
                doubles(expected[at].clone()),
                "decision line {}",
                at + 1
            );
            at += 1;
        }

        resolver.rebuild(&schema, std::slice::from_ref(record));
    }

    assert_eq!(at, expected.len());
}
