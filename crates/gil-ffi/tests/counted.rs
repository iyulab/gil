//! The records a keeper counts, through the C ABI (experimental): those outside the options' uncounted batches.

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

fn records(lines: &[Value]) -> *mut GilRecords {
    let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
    let mut out = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&text), &mut out) });
    out
}

fn settled(from: usize, to: usize, batch: &str) -> Vec<Value> {
    (from..to)
        .map(|i| {
            let team = ["network", "billing", "auth"][i % 7 % 3];
            json!({
                "id": format!("t{i}"),
                "settled_at": (i + 1) as i64 * 10_000_000,
                "values": {"component": format!("c{}", i % 7), "team": team},
                "settlements": {"team": "Enter"},
                "batch": batch,
            })
        })
        .collect()
}

#[test]
fn a_keeper_counts_the_records_outside_the_uncounted_batches() {
    let text = json!({"name": "ticket", "attributes": [
        {"name": "component", "role": "observed"},
        {"name": "team", "role": "decided"},
    ]})
    .to_string();
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&text), &mut schema) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });

    for (options, counted) in [
        (json!({"target_precision": 0.5, "minimum_answered": 5}), 150),
        (
            json!({"target_precision": 0.5, "minimum_answered": 5, "uncounted_batches": ["import"]}),
            50,
        ),
        // A name no record has leaves everything counted.
        (
            json!({"target_precision": 0.5, "minimum_answered": 5, "uncounted_batches": ["imports"]}),
            150,
        ),
    ] {
        let mut keeper = ptr::null_mut();
        ok(unsafe { gil_keeper_new(schema, s(&options.to_string()), &mut keeper) });
        for (from, to, batch) in [(0, 100, "import"), (100, 150, "live")] {
            let held = records(&settled(from, to, batch));
            ok(unsafe { gil_keeper_settle(keeper, decider, held) });
            unsafe { gil_records_free(held) };
        }
        assert_eq!(unsafe { gil_keeper_settled(keeper) }, 150);
        assert_eq!(unsafe { gil_keeper_counted(keeper) }, counted, "{options}");
        unsafe { gil_keeper_free(keeper) };
    }
    assert_eq!(unsafe { gil_keeper_counted(ptr::null()) }, 0);

    unsafe {
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}
