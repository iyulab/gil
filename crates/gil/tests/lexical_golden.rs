//! The similar document memory golden (`tests/fixtures/core/lexical.jsonl`), written by the .NET implementation: for each
//! single-valued judged field, the first 200 records remembered by their evidence, then for every later record the 11
//! documents most like its observed values — before it is remembered, with some documents forgotten and some remembered
//! again along the way — must come out the same here: sources, similarities to the bit, answers and the order of ties.

mod common;

use std::collections::BTreeMap;

use common::{CHOSEN, document, fixture, form, lines};
use gil::form::{Document, Form, Role};
use gil::lexical_memory::LexicalMemory;
use serde_json::{Value, json};

/// How many similar documents a lookup asks for: the resolver's default of ten voters, and one more for the document
/// itself.
const ASKED: usize = 11;

const FIELDS: [&str; 2] = ["team", "code"];

fn task(field: &str) -> String {
    format!("ticket/{field}")
}

/// Remembers the document for each field it holds a value of (once the values the application filled are left out),
/// and forgets it for the others.
fn put(memory: &mut LexicalMemory, form: &Form, settled: &Document) {
    let document = settled.remembered();
    for field in FIELDS {
        match document.values.get(field) {
            Some(value) => memory.remember(
                &task(field),
                &document.id,
                &form.evidence(field, &document.values, &document.sets),
                value,
            ),
            None => memory.forget(&task(field), &document.id),
        }
    }
}

#[test]
fn the_similar_document_memory_reproduces_its_golden() {
    let documents: Vec<Document> = lines(&fixture("records.jsonl"))
        .map(|r| document(&r))
        .collect();
    let form = form(&fixture("golden.jsonl"), |_| true); // the form as declared: evidence needs no chosen setting
    let expected: Vec<Value> = lines(&fixture("lexical.jsonl")).collect();

    let mut memory = LexicalMemory::default();
    for document in &documents[..CHOSEN] {
        put(&mut memory, &form, document);
    }

    let mut at = 0;
    for i in CHOSEN..documents.len() {
        let document = &documents[i];
        let observed: BTreeMap<String, String> = document
            .values
            .iter()
            .filter(|(field, _)| form.field(field).role == Role::Observed)
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        for field in FIELDS {
            let state = form.evidence(field, &observed, &BTreeMap::new());
            let matches: Vec<Value> = memory
                .nearest(&task(field), &state, ASKED)
                .into_iter()
                .map(|m| json!([m.source, m.similarity, m.answer]))
                .collect();
            let actual = json!({
                "kind": "nearest",
                "doc": document.id,
                "field": field,
                "count": memory.count(&task(field)),
                "matches": matches,
            });
            assert_eq!(actual, expected[at], "line {}", at + 1);
            at += 1;
        }

        put(&mut memory, &form, document);
        if i % 10 == 0 {
            for field in FIELDS {
                memory.forget(&task(field), &documents[i - 150].id);
            }
        }
        if i % 7 == 0 {
            put(&mut memory, &form, &documents[i - 1]); // remembered again: keeps its place among ties
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
