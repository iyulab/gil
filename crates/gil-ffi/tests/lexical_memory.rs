//! The lexical memory through the C ABI reproduces the similar record memory golden (`tests/fixtures/core/lexical.jsonl`)
//! as the core does — the same remembering, forgetting and remembering again, the same lookups, similarities to the bit.
//! Also the refusals: n-gram lengths, a count of 0, and a call without a place for its result.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::ptr;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil::schema::{AttributeRole, Record, Schema};
use gil_native::*;
use serde_json::{Value, json};

const ASKED: usize = 11;

const FIELDS: [&str; 2] = ["team", "code"];

fn s(text: &str) -> GilStr {
    GilStr {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

fn copied(copy: unsafe extern "C" fn(*mut u8, usize) -> usize) -> String {
    let len = unsafe { copy(ptr::null_mut(), 0) };
    let mut buffer = vec![0u8; len];
    assert_eq!(unsafe { copy(buffer.as_mut_ptr(), buffer.len()) }, len);
    String::from_utf8(buffer).unwrap()
}

fn ok(status: i32) {
    assert_eq!(status, GIL_OK, "{}", copied(gil_last_error));
}

fn task(attribute: &str) -> String {
    format!("ticket/{attribute}")
}

fn new_memory() -> *mut GilLexicalMemory {
    let mut memory = ptr::null_mut();
    ok(unsafe { gil_lexical_memory_new(2, 3, &mut memory) });
    memory
}

fn put(memory: *mut GilLexicalMemory, schema: &Schema, settled: &Record) {
    let record = settled.remembered();
    for attribute in FIELDS {
        let task = task(attribute);
        match record.values.get(attribute) {
            Some(value) => {
                let state = schema.evidence(attribute, &record.values, &record.sets);
                ok(unsafe {
                    gil_lexical_memory_remember(
                        memory,
                        s(&task),
                        s(&record.id),
                        s(&state),
                        s(value),
                    )
                });
            }
            None => ok(unsafe { gil_lexical_memory_forget(memory, s(&task), s(&record.id)) }),
        }
    }
}

fn count(memory: *const GilLexicalMemory, task: &str) -> usize {
    let mut count = usize::MAX;
    ok(unsafe { gil_lexical_memory_count(memory, s(task), &mut count) });
    count
}

fn nearest(memory: *const GilLexicalMemory, task: &str, state: &str, asked: usize) -> Value {
    let mut len = 0;
    ok(unsafe { gil_lexical_memory_nearest(memory, s(task), s(state), asked, &mut len) });
    serde_json::from_str(&copied(gil_last_result)).unwrap()
}

#[test]
fn the_lexical_memory_reproduces_the_similar_record_golden() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let schema = schema(&fixture("golden.jsonl"), |_| true);
    let golden = fixture("lexical.jsonl");
    let mut expected = lines(&golden);

    let memory = new_memory();
    for record in &records[..CHOSEN] {
        put(memory, &schema, record);
    }

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
            let found = nearest(memory, &task(attribute), &state, ASKED);
            let matches: Vec<Value> = found
                .as_array()
                .unwrap()
                .iter()
                .map(|m| json!([m["key"], m["similarity"], m["answer"]]))
                .collect();
            let actual = json!({
                "kind": "nearest",
                "doc": record.id,
                "field": attribute,
                "count": count(memory, &task(attribute)),
                "matches": matches,
            });
            let golden = expected
                .next()
                .expect("the golden has a line for every lookup");
            assert_eq!(actual, golden, "{} {attribute}", record.id);
        }

        put(memory, &schema, record);
        if i % 10 == 0 {
            for attribute in FIELDS {
                let (task, key) = (task(attribute), &records[i - 150].id);
                ok(unsafe { gil_lexical_memory_forget(memory, s(&task), s(key)) });
            }
        }
        if i % 7 == 0 {
            put(memory, &schema, &records[i - 1]);
        }
    }
    assert!(expected.next().is_none(), "every golden line was looked up");

    unsafe { gil_lexical_memory_free(memory) };
}

#[test]
fn an_empty_memory_finds_nothing_and_counts_none() {
    let memory = new_memory();
    assert_eq!(count(memory, "t"), 0);
    assert_eq!(nearest(memory, "t", "anything", 3), json!([]));
    unsafe { gil_lexical_memory_free(memory) };
}

#[test]
fn rules_broken_are_refused_as_input() {
    let mut memory = ptr::null_mut();
    assert_eq!(
        unsafe { gil_lexical_memory_new(0, 3, &mut memory) },
        GIL_INVALID_INPUT
    );
    assert_eq!(copied(gil_last_error), "min_gram must be at least 1");
    assert_eq!(
        unsafe { gil_lexical_memory_new(3, 2, &mut memory) },
        GIL_INVALID_INPUT
    );
    assert_eq!(copied(gil_last_error), "max_gram must be at least min_gram");
    assert!(memory.is_null());

    let memory = new_memory();
    ok(unsafe { gil_lexical_memory_remember(memory, s("t"), s("a"), s("router"), s("network")) });
    let mut len = 0;
    assert_eq!(
        unsafe { gil_lexical_memory_nearest(memory, s("t"), s("router"), 0, &mut len) },
        GIL_INVALID_INPUT
    );
    assert_eq!(copied(gil_last_error), "count must be at least 1");
    assert_eq!(
        unsafe { gil_lexical_memory_nearest(memory, s("t"), s("router"), 1, ptr::null_mut()) },
        GIL_INVALID_ARGUMENT
    );
    assert_eq!(
        unsafe { gil_lexical_memory_count(ptr::null(), s("t"), &mut len) },
        GIL_INVALID_ARGUMENT
    );
    // A refusal changes nothing.
    assert_eq!(count(memory, "t"), 1);
    unsafe { gil_lexical_memory_free(memory) };
    unsafe { gil_lexical_memory_free(ptr::null_mut()) };
}
