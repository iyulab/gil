//! A judge outside Gil's threshold through the C ABI (experimental): scores and what people settled in, the replay out.

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

/// 4,000 decisions, a score that is right `score` of the time: the right ones are those whose index-derived draw falls
/// under the score (SplitMix64 so the file is the same on every run).
fn decisions() -> Value {
    let mut state = 17_u64;
    let mut next = move || {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
    };
    Value::Array(
        (0..4_000)
            .map(|i| {
                let score = next();
                let right = next() < score;
                json!({"id": format!("c{i:05}"), "settled_at": i, "score": score, "right": right})
            })
            .collect(),
    )
}

fn run(decisions: &Value, options: &Value) -> (i32, String) {
    let (d, o) = (decisions.to_string(), options.to_string());
    let mut len = 0usize;
    let status = unsafe { gil_select_score_threshold(s(&d), s(&o), &mut len) };
    let text = if status == GIL_OK {
        copied(gil_last_result)
    } else {
        copied(gil_last_error)
    };
    (status, text)
}

#[test]
fn scores_and_outcomes_in_give_a_threshold_and_a_promise_out() {
    let (status, text) = run(
        &decisions(),
        &json!({"target_precision": 0.8, "minimum_answered": 30}),
    );
    assert_eq!(status, GIL_OK, "{text}");
    let replay: Value = serde_json::from_str(&text).unwrap();
    let chosen = &replay["chosen"];
    let threshold = chosen["threshold"].as_f64().unwrap();
    assert!((0.7..=0.9).contains(&threshold), "{threshold}");
    assert!(chosen["precision"].as_f64().unwrap() >= 0.8);
    assert!(chosen["promise"].as_f64().unwrap() <= chosen["precision"].as_f64().unwrap());
    assert_eq!(chosen["lookups"], 4_000);
}

#[test]
fn a_decision_with_no_score_is_asked_and_not_answered() {
    let mut all = decisions();
    all.as_array_mut()
        .unwrap()
        .push(json!({"id": "silent", "settled_at": 9_999}));
    let (status, text) = run(
        &all,
        &json!({"target_precision": 0.8, "minimum_answered": 30}),
    );
    assert_eq!(status, GIL_OK, "{text}");
    let replay: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(replay["chosen"]["lookups"], 4_001);
}

#[test]
fn no_threshold_is_null_not_an_error() {
    let flat: Vec<Value> = (0..400)
        .map(|i| json!({"id": format!("c{i}"), "settled_at": i, "score": 0.5, "right": i % 2 == 0}))
        .collect();
    let (status, text) = run(
        &Value::Array(flat),
        &json!({"target_precision": 0.9, "minimum_answered": 30}),
    );
    assert_eq!(status, GIL_OK, "{text}");
    let replay: Value = serde_json::from_str(&text).unwrap();
    assert!(replay["chosen"].is_null());
}

#[test]
fn what_does_not_read_is_refused_as_input() {
    let (status, _) = run(
        &json!({"not": "an array"}),
        &json!({"target_precision": 0.8, "minimum_answered": 1}),
    );
    assert_eq!(status, GIL_INVALID_INPUT);
    let (status, _) = run(
        &decisions(),
        &json!({"target_precision": 0.8, "minimum_answered": 1, "mystery": 1}),
    );
    assert_eq!(status, GIL_INVALID_INPUT);
}

#[test]
fn a_score_that_is_not_a_finite_number_is_refused_as_input() {
    // 1e999 overflows a double, so the JSON does not read; the core never sees an infinite score.
    let text = r#"[{"id": "a", "settled_at": 0, "score": 1e999, "right": true}]"#;
    let options = json!({"target_precision": 0.8, "minimum_answered": 1}).to_string();
    let mut len = 0usize;
    let status = unsafe { gil_select_score_threshold(s(text), s(&options), &mut len) };
    assert_eq!(status, GIL_INVALID_INPUT);
}
