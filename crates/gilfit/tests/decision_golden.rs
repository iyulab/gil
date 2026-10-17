//! The decision lines of the form path's golden (`tests/fixtures/core/golden.jsonl`, kind `decision`), written by the
//! .NET implementation: a resolver rebuilt from the first 200 records with the settings the golden chose, asked about
//! every judged field of each later record from its observed values before that record is put, must suggest the same
//! here — whether it answered, the first candidate's layer, the first three candidates (values, scores to the bit,
//! layers, trust) and the coarse class.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, document, doubles, fixture, form, lines};
use gilfit::field_memory::FieldMemory;
use gilfit::form::{Document, Role};
use gilfit::form_resolver::{Candidate, FormResolver};
use gilfit::lexical_memory::LexicalMemory;
use serde_json::{Value, json};

fn candidate(c: &Candidate) -> Value {
    json!([c.value, c.score, c.source.name(), c.trusted])
}

#[test]
fn the_resolver_reproduces_the_golden_decisions() {
    let documents: Vec<Document> = lines(&fixture("records.jsonl"))
        .map(|r| document(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let form = form(&golden, |_| false);
    let expected: Vec<Value> = lines(&golden).filter(|l| l["kind"] == "decision").collect();

    let mut resolver = FormResolver::new(FieldMemory::default(), Some(LexicalMemory::default()));
    resolver.rebuild(&form, &documents[..CHOSEN]);
    let mut at = 0;
    for document in &documents[CHOSEN..] {
        let observed: BTreeMap<String, String> = document
            .values
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        for suggestion in resolver.suggest(&form, &document.id, &observed, &BTreeMap::new()) {
            let actual = json!({
                "kind": "decision",
                "doc": document.id,
                "field": suggestion.field,
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

        resolver.rebuild(&form, std::slice::from_ref(document));
    }

    assert_eq!(at, expected.len());
}
