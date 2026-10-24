//! A keeper that starts again under a new edition of the code lists, through the C ABI (experimental).

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

/// Records `from..to` under `edition`; the second edition renumbers the teams.
fn settled(from: usize, to: usize, edition: &str) -> Vec<Value> {
    let teams = ["network", "billing", "auth"];
    (from..to)
        .map(|i| {
            let team = teams[i % 7 % 3];
            json!({
                "id": format!("t{i}"),
                "settled_at": (i + 1) as i64 * 10_000_000,
                "values": {
                    "component": format!("c{}", i % 7),
                    "team": if edition == "2" { format!("{team}-2") } else { team.to_owned() },
                },
                "settlements": {"team": "Enter"},
                "form_version": edition,
            })
        })
        .collect()
}

#[test]
fn a_keeper_told_to_restart_keeps_the_new_edition_alone() {
    let text = json!({"name": "ticket", "attributes": [
        {"name": "component", "role": "observed"},
        {"name": "team", "role": "decided"},
    ]})
    .to_string();
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&text), &mut schema) });
    let options =
        json!({"target_precision": 0.5, "minimum_answered": 5, "restart_on_form_version": true})
            .to_string();
    let mut keeper = ptr::null_mut();
    ok(unsafe { gil_keeper_new(schema, s(&options), &mut keeper) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });

    let mut len = 0;
    for (from, to, edition) in [(0, 200, "1"), (200, 300, "2"), (300, 310, "1")] {
        let held = records(&settled(from, to, edition));
        ok(unsafe { gil_keeper_settle(keeper, decider, held) });
        unsafe { gil_records_free(held) };
        if edition == "1" && from == 0 {
            let state = result(unsafe { gil_keeper_rechoose(keeper, false, &mut len) });
            assert_eq!(state["form_version"], json!("1"));
            assert!(state["chosen"]["team"].is_object());
        }
    }
    // The old edition's late records are not taken, and the new edition starts with nothing chosen.
    assert_eq!(unsafe { gil_keeper_settled(keeper) }, 100);
    let state = result(unsafe { gil_keeper_state(keeper, &mut len) });
    assert_eq!(state["form_version"], json!("2"));
    assert_eq!(state["chosen"], json!({}));
    let state = result(unsafe { gil_keeper_rechoose(keeper, false, &mut len) });
    assert!(state["chosen"]["team"].is_object(), "{state}");

    unsafe {
        gil_keeper_free(keeper);
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}
