//! A keeper that says when the answers stopped keeping the promise, through the C ABI (experimental).

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

fn records(lines: &[Value]) -> *mut GilRecords {
    let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
    let mut out = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&text), &mut out) });
    out
}

/// Records `from..to` whose team answer was accepted, or corrected when `right` says no.
fn judged(from: usize, to: usize, right: impl Fn(usize) -> bool) -> Vec<Value> {
    (from..to)
        .map(|i| {
            json!({
                "id": format!("t{i}"),
                "settled_at": (i + 1) as i64 * 10_000_000,
                "values": {"component": format!("c{}", i % 7), "team": "network"},
                "settlements": {"team": if right(i) { "Accept" } else { "Correct" }},
            })
        })
        .collect()
}

#[test]
fn a_keeper_told_to_watch_names_the_attribute_whose_answers_fell_short() {
    let text = json!({"name": "ticket", "attributes": [
        {"name": "component", "role": "observed"},
        {"name": "team", "role": "decided"},
    ]})
    .to_string();
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&text), &mut schema) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });

    let mut len = 0;
    for (watch, expected) in [
        (
            true,
            json!({"realized": {"attribute": "team", "right": 50, "judged": 100}}),
        ),
        (false, Value::Null),
    ] {
        let options =
            json!({"target_precision": 0.9, "minimum_answered": 5, "watch_realized": watch})
                .to_string();
        let mut keeper = ptr::null_mut();
        ok(unsafe { gil_keeper_new(schema, s(&options), &mut keeper) });
        let held = records(&judged(0, 100, |i| i % 2 == 0));
        ok(unsafe { gil_keeper_settle(keeper, decider, held) });
        unsafe { gil_records_free(held) };
        result(unsafe { gil_keeper_rechoose(keeper, false, &mut len) });
        assert_eq!(
            result(unsafe { gil_keeper_stale(keeper, &mut len) }),
            expected
        );
        unsafe { gil_keeper_free(keeper) };
    }

    unsafe {
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}
