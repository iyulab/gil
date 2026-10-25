//! A keeper's unseen mass through the C ABI (experimental).

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

fn result(status: i32) -> Value {
    assert_eq!(status, GIL_OK, "{}", copied(gil_last_error));
    serde_json::from_str(&copied(gil_last_result)).unwrap()
}

#[test]
fn a_keeper_says_how_many_values_were_settled_once() {
    let text = json!({"name": "ticket", "attributes": [
        {"name": "component", "role": "observed"},
        {"name": "team", "role": "decided"},
    ]})
    .to_string();
    let mut schema = ptr::null_mut();
    assert_eq!(unsafe { gil_schema_new(s(&text), &mut schema) }, GIL_OK);
    let options = json!({"target_precision": 0.5, "minimum_answered": 5}).to_string();
    let mut keeper = ptr::null_mut();
    assert_eq!(
        unsafe { gil_keeper_new(schema, s(&options), &mut keeper) },
        GIL_OK
    );
    let mut decider = ptr::null_mut();
    assert_eq!(unsafe { gil_decider_new(true, 0, &mut decider) }, GIL_OK);

    let mut len = 0;
    assert_eq!(
        result(unsafe { gil_keeper_unseen_mass(keeper, s("team"), &mut len) }),
        Value::Null
    );
    let lines: String = ["network", "network", "billing", "auth"]
        .iter()
        .enumerate()
        .map(|(i, team)| {
            json!({"id": format!("t{i}"), "settled_at": i as i64 + 1,
                   "values": {"component": "c", "team": team}})
            .to_string()
                + "\n"
        })
        .collect();
    let mut held = ptr::null_mut();
    assert_eq!(unsafe { gil_records_new(s(&lines), &mut held) }, GIL_OK);
    assert_eq!(unsafe { gil_keeper_settle(keeper, decider, held) }, GIL_OK);
    let mass = result(unsafe { gil_keeper_unseen_mass(keeper, s("team"), &mut len) });
    assert_eq!(mass, json!({"once": 2, "settled": 4, "mass": 0.5}));
    assert_eq!(
        result(unsafe { gil_keeper_unseen_mass(keeper, s("component"), &mut len) }),
        Value::Null
    );

    unsafe {
        gil_records_free(held);
        gil_keeper_free(keeper);
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}
