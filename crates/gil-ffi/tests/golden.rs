//! The C ABI reproduces the decision lines of the schema path's golden (`tests/fixtures/core/golden.jsonl`, kind
//! `decision`) — the same walk as the core's own decision golden, every schema, record and answer through the handles,
//! JSON texts and caller buffers a binding uses: a decider filled once from the first 200 logs, asked about each later
//! log from its observed values, then given that log. Also: a refusal comes back as a status and a message, and a malformed
//! call never reaches the core.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::ptr;

use common::{CHOSEN, doubles, fixture, lines, schema, to_record};
use gil::schema::{AttributeRole, Record};
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
    let written = unsafe { copy(buffer.as_mut_ptr(), buffer.len()) };
    assert_eq!(written, len);
    String::from_utf8(buffer).unwrap()
}

fn last_error() -> String {
    copied(gil_last_error)
}

fn ok(status: i32) {
    assert_eq!(status, GIL_OK, "{}", last_error());
    assert_eq!(last_error(), "");
}

fn new_schema(json: &str) -> *mut GilSchema {
    let mut out = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(json), &mut out) });
    assert!(!out.is_null());
    out
}

fn new_decider() -> *mut GilDecider {
    let mut out = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, &mut out) });
    out
}

fn put(decider: *mut GilDecider, schema: *const GilSchema, records: &[Record]) {
    let lines: String = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect();
    ok(unsafe { gil_decider_put(decider, schema, s(&lines)) });
}

fn answer(
    decider: *const GilDecider,
    schema: *const GilSchema,
    id: &str,
    values: &str,
    sets: &str,
) -> Value {
    let mut len = usize::MAX;
    ok(unsafe { gil_decider_answer(decider, schema, s(id), s(values), s(sets), &mut len) });
    let text = copied(gil_last_result);
    assert_eq!(text.len(), len);
    serde_json::from_str(&text).unwrap()
}

fn candidate(c: &Value) -> Value {
    json!([c["value"], c["score"], c["source"], c["trusted"]])
}

fn golden_schema() -> String {
    serde_json::to_string(&schema(&fixture("golden.jsonl"), |_| false)).unwrap()
}

#[test]
fn the_c_abi_reproduces_the_golden_decisions() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let golden = fixture("golden.jsonl");
    let schema_json = golden_schema();
    let core_schema = schema(&golden, |_| false);
    let expected: Vec<Value> = lines(&golden).filter(|l| l["kind"] == "decision").collect();

    let schema = new_schema(&schema_json);
    let decider = new_decider();
    put(decider, schema, &records[..CHOSEN]);
    let mut at = 0;
    for record in &records[CHOSEN..] {
        let observed: BTreeMap<&String, &String> = record
            .values
            .iter()
            .filter(|(attribute, _)| {
                core_schema.attribute(attribute).role == AttributeRole::Observed
            })
            .collect();
        let values = serde_json::to_string(&observed).unwrap();
        let answers = answer(decider, schema, &record.id, &values, "");
        for suggestion in answers.as_array().unwrap() {
            let actual = json!({
                "kind": "decision",
                "doc": record.id,
                "field": suggestion["attribute"],
                "answered": suggestion["answered"],
                "source": suggestion["source"],
                "candidates": suggestion["candidates"].as_array().unwrap().iter().map(candidate).collect::<Vec<_>>(),
                "coarse": if suggestion["coarse"].is_null() { Value::Null } else { candidate(&suggestion["coarse"]) },
            });
            assert_eq!(
                doubles(actual),
                doubles(expected[at].clone()),
                "decision line {}",
                at + 1
            );
            at += 1;
        }

        put(decider, schema, std::slice::from_ref(record));
    }
    assert_eq!(at, expected.len());

    unsafe {
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}

#[test]
fn a_refusal_is_a_status_and_a_message() {
    let mut out = ptr::null_mut();
    let status = unsafe { gil_schema_new(s("{\"name\":"), &mut out) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(last_error().starts_with("a schema does not read"));
    assert!(out.is_null());

    // A schema that reads but does not check: an evidence attribute it does not hold.
    let mut bad: Value = serde_json::from_str(&golden_schema()).unwrap();
    let decided = bad["attributes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|a| a["role"] == "decided")
        .unwrap();
    decided["depends_on"] = json!(["no-such-attribute"]);
    let status = unsafe { gil_schema_new(s(&bad.to_string()), &mut out) };
    assert_eq!(status, GIL_INVALID_INPUT, "{}", last_error());
    assert!(!last_error().is_empty());

    let schema = new_schema(&golden_schema());
    let decider = new_decider();
    let status = unsafe { gil_decider_put(decider, schema, s("\n{\"id\":\"a\"}\n")) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(
        last_error().starts_with("record line 2 does not read"),
        "{}",
        last_error()
    );

    // The next call that succeeds clears the message.
    put(decider, schema, &[]);
    unsafe {
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}

#[test]
fn a_malformed_call_never_reaches_the_core() {
    let schema = new_schema(&golden_schema());
    let decider = new_decider();
    let mut len = 0;

    let status = unsafe { gil_schema_new(s("{}"), ptr::null_mut()) };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert_eq!(last_error(), "out is null");

    let status =
        unsafe { gil_decider_answer(ptr::null(), schema, s("a"), s("{}"), s(""), &mut len) };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert_eq!(last_error(), "decider is null");

    let status = unsafe { gil_decider_answer(decider, schema, s("a"), s("[]"), s(""), &mut len) };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert!(last_error().starts_with("values are a JSON object of strings"));

    let not_utf8 = [0xffu8, 0xfe];
    let bad = GilStr {
        ptr: not_utf8.as_ptr(),
        len: not_utf8.len(),
    };
    let status = unsafe { gil_decider_answer(decider, schema, bad, s("{}"), s(""), &mut len) };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert_eq!(last_error(), "record_id is not UTF-8");

    let status = unsafe {
        gil_decider_put(
            decider,
            schema,
            GilStr {
                ptr: ptr::null(),
                len: 3,
            },
        )
    };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert_eq!(last_error(), "records is null but its length is 3");

    let status =
        unsafe { gil_decider_answer(decider, schema, s("a"), s("{}"), s(""), ptr::null_mut()) };
    assert_eq!(status, GIL_INVALID_ARGUMENT);
    assert_eq!(last_error(), "result_len is null");
    assert_eq!(copied(gil_last_result), "");

    // Freeing null does nothing.
    unsafe {
        gil_decider_free(ptr::null_mut());
        gil_schema_free(ptr::null_mut());
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}
