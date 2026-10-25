//! A keeper's similar elements through the C ABI (experimental): the elements of an attribute that takes several values
//! that the similar record layer offers, with the share of each and the promise.

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

fn desk(elements: bool) -> String {
    let tags = if elements {
        json!({"name": "tags", "role": "decided", "multiple": true, "similar_elements": true})
    } else {
        json!({"name": "tags", "role": "decided", "multiple": true})
    };
    json!({"name": "desk", "attributes": [{"name": "text", "role": "observed"}, tags]}).to_string()
}

fn records() -> String {
    (0..240)
        .map(|i| {
            let topic = i % 4;
            json!({
                "id": format!("r{i:03}"),
                "settled_at": (i + 1) as i64 * 10_000_000,
                "values": {"text": format!("topic{topic}x w{} topic{topic}y", (i * 7919) % 97)},
                "sets": {"tags": [format!("core{topic}a"), format!("core{topic}b")]},
            })
            .to_string()
                + "\n"
        })
        .collect()
}

#[test]
fn a_keeper_offers_the_elements_the_similar_records_agree_on_with_the_promise() {
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&desk(true)), &mut schema) });
    let options = json!({"target_precision": 0.9, "minimum_answered": 10}).to_string();
    let mut keeper = ptr::null_mut();
    ok(unsafe { gil_keeper_new(schema, s(&options), &mut keeper) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });
    let lines = records();
    let mut held = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&lines), &mut held) });
    ok(unsafe { gil_keeper_settle(keeper, decider, held) });

    let mut len = 0;
    let asked = |len: &mut usize| {
        result(unsafe {
            gil_keeper_similar_elements(
                keeper,
                decider,
                s("asking"),
                s("tags"),
                s(r#"{"text": "topic2x w5 topic2y"}"#),
                s(""),
                len,
            )
        })
    };
    // Before a choice there is no threshold, and so no element.
    assert_eq!(asked(&mut len), json!([]));

    let state = result(unsafe { gil_keeper_rechoose(keeper, false, &mut len) });
    let tuned = &state["tuning"]["attributes"][0];
    assert!(tuned["memory_threshold"].is_number(), "{tuned}");
    let found = asked(&mut len);
    let found = found.as_array().unwrap();
    let values: Vec<&str> = found.iter().map(|e| e["value"].as_str().unwrap()).collect();
    assert_eq!(values, ["core2a", "core2b"]);
    for element in found {
        assert!(element["share"].as_f64().unwrap() > 0.9);
        assert_eq!(element["promise"], tuned["memory_promise"]);
    }

    // The same schema without the setting writes none of it, and the keeper chooses nothing for the set.
    let mut plain = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&desk(false)), &mut plain) });
    let mut other = ptr::null_mut();
    ok(unsafe { gil_keeper_new(plain, s(&options), &mut other) });
    let state = result(unsafe { gil_keeper_state(other, &mut len) });
    assert_eq!(state["chosen"], json!({}));
    let mut written = 0;
    let schema_json = result(unsafe { gil_schema_json(plain, &mut written) }).to_string();
    assert!(!schema_json.contains("similar_elements"), "{schema_json}");
    // Asking for what is not answered that way is a contract violation.
    assert_eq!(
        unsafe {
            gil_keeper_similar_elements(
                other,
                decider,
                s("asking"),
                s("tags"),
                s("{}"),
                s(""),
                &mut len,
            )
        },
        GIL_PANIC
    );

    unsafe {
        gil_records_free(held);
        gil_keeper_free(keeper);
        gil_keeper_free(other);
        gil_decider_free(decider);
        gil_schema_free(schema);
        gil_schema_free(plain);
    }
}
