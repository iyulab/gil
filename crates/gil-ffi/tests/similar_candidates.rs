//! The similar record layer's runner-up values as candidates through the C ABI (experimental): the setting travels in the
//! schema's JSON, a schema that leaves it off writes none of it, and an answer carries the runners-up after the value the
//! vote chose, never trusted.

use std::ptr;

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

fn ok(status: i32) {
    assert_eq!(status, GIL_OK, "{}", copied(gil_last_error));
}

fn result(status: i32) -> Value {
    ok(status);
    serde_json::from_str(&copied(gil_last_result)).unwrap()
}

fn intake(candidates: Option<usize>) -> String {
    let mut category = json!({"name": "category", "role": "decided", "memory_threshold": 0.0});
    if let Some(n) = candidates {
        category["similar_candidates"] = json!(n);
    }
    json!({"name": "intake", "attributes": [{"name": "note", "role": "observed"}, category]})
        .to_string()
}

fn records() -> String {
    [
        (
            "router keeps dropping the connection every evening",
            "billing",
        ),
        ("router keeps dropping the connection at night", "outage"),
        ("router keeps dropping connection in the evening", "outage"),
        (
            "the router is dropping the connection every night",
            "outage",
        ),
        ("invoice shows a charge twice this month", "billing"),
        (
            "router connection dropped and the invoice was refunded",
            "refund",
        ),
    ]
    .iter()
    .zip(1..)
    .map(|((note, category), n)| {
        json!({
            "id": format!("d{n}"),
            "settled_at": n as i64 * 10_000_000,
            "values": {"note": note, "category": category},
        })
        .to_string()
            + "\n"
    })
    .collect()
}

/// The answer for the asked note, through a decider that holds the records.
fn answered(schema_json: &str) -> Value {
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(schema_json), &mut schema) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });
    let lines = records();
    let mut held = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&lines), &mut held) });
    ok(unsafe { gil_decider_put(decider, schema, held) });
    let mut len = 0;
    let found = result(unsafe {
        gil_decider_answer(
            decider,
            schema,
            s("d9"),
            s(r#"{"note": "router keeps dropping the connection every evening again"}"#),
            s(""),
            false,
            &mut len,
        )
    });
    unsafe {
        gil_records_free(held);
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
    found
}

fn from_similar(answer: &Value) -> Vec<&Value> {
    answer["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["source"] == "SimilarRecord")
        .collect()
}

#[test]
fn the_setting_travels_in_the_schema_and_the_runners_up_come_back_untrusted() {
    let off = answered(&intake(None));
    let on = answered(&intake(Some(3)));
    let (off, on) = (&off[0], &on[0]);

    assert_eq!(from_similar(off).len(), 1);
    assert_eq!(on["candidates"][0], off["candidates"][0]);
    assert_eq!(on["candidates"][0]["trusted"], true);
    let found = from_similar(on);
    let values: Vec<&str> = found.iter().map(|c| c["value"].as_str().unwrap()).collect();
    assert_eq!(values, ["outage", "billing", "refund"]);
    for runner_up in &found[1..] {
        assert_eq!(runner_up["trusted"], false, "{runner_up}");
        assert!(runner_up["score"].as_f64().unwrap() > 0.0);
        assert!(
            runner_up["evidence"]["record"]["id"].is_string(),
            "{runner_up}"
        );
    }
}

#[test]
fn a_schema_that_leaves_it_off_writes_none_of_it() {
    let mut off = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&intake(None)), &mut off) });
    let mut on = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&intake(Some(3))), &mut on) });
    let mut len = 0;
    let off_json = result(unsafe { gil_schema_json(off, &mut len) }).to_string();
    let on_json = result(unsafe { gil_schema_json(on, &mut len) }).to_string();
    assert!(!off_json.contains("similar_candidates"), "{off_json}");
    assert!(on_json.contains("\"similar_candidates\":3"), "{on_json}");
    unsafe {
        gil_schema_free(off);
        gil_schema_free(on);
    }
}

#[test]
fn only_a_decided_attribute_with_a_single_value_takes_it() {
    let bad = json!({"name": "intake", "attributes": [
        {"name": "note", "role": "observed", "similar_candidates": 3},
        {"name": "category", "role": "decided"},
    ]})
    .to_string();
    let mut schema = ptr::null_mut();
    assert_ne!(unsafe { gil_schema_new(s(&bad), &mut schema) }, GIL_OK);
    assert!(copied(gil_last_error).contains("similar candidates"));
}
