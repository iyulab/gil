//! The first record of a batch through the C ABI (experimental): an open record and a decider say what the core says,
//! the schema views and the tuning's promise are the core's, and a keeper chooses the first records apart.

use std::ptr;

use gil_native::*;
use serde_json::{Value, json};

fn s(text: &str) -> GilStr {
    GilStr {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

const NONE: GilStr = GilStr {
    ptr: ptr::null(),
    len: 0,
};

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

fn schema(json: &Value) -> *mut GilSchema {
    let text = json.to_string();
    let mut out = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&text), &mut out) });
    out
}

fn schema_json(handle: *const GilSchema) -> Value {
    let mut len = 0;
    result(unsafe { gil_schema_json(handle, &mut len) })
}

fn records(lines: &[Value]) -> *mut GilRecords {
    let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
    let mut out = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&text), &mut out) });
    out
}

fn settled(n: usize, size: usize) -> Vec<Value> {
    let teams = ["network", "billing", "auth"];
    (0..n)
        .map(|i| {
            json!({
                "id": format!("t{i}"),
                "settled_at": (i + 1) as i64 * 10_000_000,
                "values": {"component": format!("c{}", i % 7), "team": teams[i % 7 % 3]},
                "settlements": {"team": "Enter"},
                "batch": format!("b{}", i / size),
            })
        })
        .collect()
}

fn attributes(first: Option<Value>) -> Value {
    let mut team = json!({"name": "team", "role": "decided"});
    if let Some(first) = first {
        team["first_of_batch"] = first;
    }
    json!({"name": "ticket", "attributes": [{"name": "component", "role": "observed"}, team]})
}

#[test]
fn an_open_record_and_a_decider_say_whether_a_record_is_the_first_of_its_batch() {
    let schema = schema(&attributes(Some(json!({"key_threshold": 0.0}))));
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });
    let held = records(&settled(3, 1));
    ok(unsafe { gil_decider_put(decider, schema, held) });

    let mut first = false;
    ok(unsafe { gil_decider_is_first_of_batch(decider, s("b9"), NONE, &mut first) });
    assert!(first);
    ok(unsafe { gil_decider_is_first_of_batch(decider, s("b1"), NONE, &mut first) });
    assert!(!first);
    ok(unsafe { gil_decider_is_first_of_batch(decider, s("b1"), s("t1"), &mut first) });
    assert!(first);
    ok(unsafe { gil_decider_is_first_of_batch(decider, NONE, NONE, &mut first) });
    assert!(first);

    for (batch, expected) in [("b9", true), ("b1", false)] {
        let mut record = ptr::null_mut();
        ok(unsafe { gil_open_record_new(schema, s("n1"), 99, s(batch), &mut record) });
        ok(unsafe { gil_open_record_is_first_of_batch(record, decider, &mut first) });
        assert_eq!(first, expected, "{batch}");
        let mut len = 0;
        let snapshot = result(unsafe { gil_open_record_snapshot(record, &mut len) });
        assert_eq!(snapshot["batch"], json!(batch));
        unsafe { gil_open_record_free(record) };
    }

    unsafe {
        gil_records_free(held);
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}

#[test]
fn a_keeper_chooses_the_first_records_apart_and_the_schema_and_tuning_carry_them() {
    let plain = schema(&attributes(None));
    let mut keeper = ptr::null_mut();
    let options =
        json!({"target_precision": 0.5, "minimum_answered": 5, "first_of_batch": true}).to_string();
    ok(unsafe { gil_keeper_new(plain, s(&options), &mut keeper) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });
    let held = records(&settled(400, 4));
    ok(unsafe { gil_keeper_settle(keeper, decider, held) });
    let mut len = 0;
    let state = result(unsafe { gil_keeper_rechoose(keeper, false, &mut len) });
    let first = &state["first_of_batch"]["team"];
    let chosen = &first["key"]["chosen"];
    assert!(chosen.is_object(), "{state}");

    let thresholds = first.to_string();
    let mut with = ptr::null_mut();
    ok(unsafe { gil_schema_with_first_of_batch(plain, s("team"), s(&thresholds), &mut with) });
    let team = &schema_json(with)["attributes"][1];
    assert_eq!(team["first_of_batch"]["key_threshold"], chosen["threshold"]);

    let mut view = ptr::null_mut();
    ok(unsafe { gil_schema_for_first_of_batch(with, &mut view) });
    assert_eq!(
        schema_json(view)["attributes"][1]["key_threshold"],
        chosen["threshold"]
    );

    let mut len = 0;
    let captured = result(unsafe { gil_tuning_capture(with, 400, 0.5, &mut len) }).to_string();
    let promised = result(unsafe {
        gil_tuning_promise_first_of_batch(s(&captured), s("team"), s(&thresholds), &mut len)
    });
    assert_eq!(
        promised["attributes"][0]["first_key_promise"],
        chosen["promise"]
    );
    assert_eq!(
        state["tuning"]["attributes"][0]["first_key_promise"],
        chosen["promise"]
    );

    unsafe {
        gil_schema_free(view);
        gil_schema_free(with);
        gil_records_free(held);
        gil_decider_free(decider);
        gil_keeper_free(keeper);
        gil_schema_free(plain);
    }
}
