//! A value the application filled is neither learned nor asked about by a selection — through the C ABI. The golden
//! records with fills among them, filled copies of the first hundred, give the thresholds the golden records alone give
//! (the core's `fill_invariant` holds the same of the core; this holds the C ABI to it, which .NET and Python read).

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::ptr;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil::schema::{Record, SettlementKind};
use gil_native::*;
use serde_json::{Value, json};

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

/// The thresholds a layers selection of `attribute` over `records` gives, in the C ABI's JSON.
fn thresholds(schema_json: &str, records: &[Record], attribute: &str, options: &Value) -> Value {
    let text: String = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect();
    let (mut schema, mut settled) = (ptr::null_mut(), ptr::null_mut());
    assert_eq!(
        unsafe { gil_schema_new(s(schema_json), &mut schema) },
        GIL_OK
    );
    assert_eq!(unsafe { gil_records_new(s(&text), &mut settled) }, GIL_OK);
    let (options, mut len) = (options.to_string(), 0);
    let status = unsafe { gil_select_layers(schema, s(attribute), settled, s(&options), &mut len) };
    assert_eq!(status, GIL_OK, "{}", copied(gil_last_error));
    let found: Value = serde_json::from_str(&copied(gil_last_result)).unwrap();
    unsafe {
        gil_records_free(settled);
        gil_schema_free(schema);
    }
    found["thresholds"].clone()
}

#[test]
fn a_filled_value_is_neither_learned_nor_asked_through_the_c_abi() {
    let golden = schema(&fixture("golden.jsonl"), |_| false);
    let schema_json = serde_json::to_string(&golden).unwrap();
    let people: Vec<Record> = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect();
    let fills: Vec<Record> = people
        .iter()
        .take(100)
        .map(|record| {
            let mut filled = record.clone();
            filled.id = format!("fill-{}", record.id);
            filled.settled_at += 1;
            filled
                .settlements
                .insert("team".to_owned(), SettlementKind::Fill);
            filled
        })
        .collect();
    let both: Vec<Record> = people.iter().cloned().chain(fills).collect();

    let options = json!({"target_precision": 0.8, "minimum_answered": 10});
    let alone = thresholds(&schema_json, &people, "team", &options);
    let mixed = thresholds(&schema_json, &both, "team", &options);
    assert!(
        alone["key"]["chosen"].is_object() || alone["memory"]["chosen"].is_object(),
        "a stream that chooses something, or the comparison says nothing"
    );
    assert_eq!(mixed, alone);
}
