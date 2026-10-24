//! The fixtures a binding's own tests read (`tests/fixtures/core/native/`): the schema path's golden schema and its
//! records in the core's `serde` form — what `gil_schema_new` and `gil_records_new` take — written from the same
//! sources the core's golden tests read, and `selections.json`: calls of the selections, the tuning round trip and the
//! usage counts through this C ABI, each with its inputs and the result it gave (the `selection` tests hold those
//! results to the core's own). A binding that reproduces the file passes its inputs through as the C ABI reads them.
//! This test fails when they are out of date; `GIL_WRITE_FIXTURES=1` rewrites them.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::ptr;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil_native::*;
use serde_json::{Value, json};

/// A tuning with its version (the crate's, `gil::tuning::VERSION`) replaced by `VERSION_PLACEHOLDER`, so the fixture
/// does not change with every release; .NET puts its own back before comparing.
fn versionless(mut tuning: Value) -> Value {
    assert_eq!(tuning["version"], gil::tuning::VERSION);
    tuning["version"] = Value::from(VERSION_PLACEHOLDER);
    tuning
}

const VERSION_PLACEHOLDER: &str = "{version}";

fn check_or_write(name: &str, expected: &str) {
    let path = format!(
        "{}/../../tests/fixtures/core/native/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    if std::env::var_os("GIL_WRITE_FIXTURES").is_some() {
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        std::fs::write(&path, expected).unwrap();
        return;
    }
    let actual = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{name} is missing; run with GIL_WRITE_FIXTURES=1"));
    assert!(
        actual == expected,
        "{name} is out of date; run with GIL_WRITE_FIXTURES=1"
    );
}

#[test]
fn the_native_fixtures_are_up_to_date() {
    let golden_schema = schema(&fixture("golden.jsonl"), |_| false);
    check_or_write(
        "schema.json",
        &(serde_json::to_string_pretty(&golden_schema).unwrap() + "\n"),
    );
    let records: String = lines(&fixture("records.jsonl"))
        .map(|r| serde_json::to_string(&to_record(&r)).unwrap() + "\n")
        .collect();
    check_or_write("records.jsonl", &records);
}

fn s(text: &str) -> GilStr {
    GilStr {
        ptr: text.as_ptr(),
        len: text.len(),
    }
}

fn opt(text: Option<&str>) -> GilStr {
    text.map_or(
        GilStr {
            ptr: ptr::null(),
            len: 0,
        },
        s,
    )
}

fn copied(copy: unsafe extern "C" fn(*mut u8, usize) -> usize) -> String {
    let len = unsafe { copy(ptr::null_mut(), 0) };
    let mut buffer = vec![0u8; len];
    assert_eq!(unsafe { copy(buffer.as_mut_ptr(), buffer.len()) }, len);
    String::from_utf8(buffer).unwrap()
}

/// The JSON result of the call just made.
fn result(status: i32) -> Value {
    assert_eq!(status, GIL_OK, "{}", copied(gil_last_error));
    serde_json::from_str(&copied(gil_last_result)).unwrap()
}

/// A usage decision as given: answer, settled value, settlement kind, probability, promise.
type Given = (
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
    f64,
    Option<f64>,
);

const COUNTING: [&str; 6] = [
    "Enter",
    "Accept",
    "Correct",
    "Check",
    "AcceptOther",
    "CorrectOther",
];

#[test]
fn the_selections_fixture_is_up_to_date() {
    let golden_schema = schema(&fixture("golden.jsonl"), |_| false);
    let schema_json = serde_json::to_string(&golden_schema).unwrap();
    let records: String = lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| serde_json::to_string(&to_record(&r)).unwrap() + "\n")
        .collect();
    let (mut schema, mut settled) = (ptr::null_mut(), ptr::null_mut());
    assert_eq!(
        unsafe { gil_schema_new(s(&schema_json), &mut schema) },
        GIL_OK
    );
    assert_eq!(
        unsafe { gil_records_new(s(&records), &mut settled) },
        GIL_OK
    );
    let mut len = 0;
    let mut calls = Vec::new();

    let selections = [
        (
            "layers",
            "team",
            json!({"target_precision": 0.8, "minimum_answered": 10, "counting": COUNTING}),
        ),
        (
            "layers",
            "code",
            json!({"target_precision": 0.7, "minimum_answered": 5, "rule": "pooled", "decisions": true}),
        ),
        (
            "coarse",
            "code",
            json!({"target_precision": 0.8, "minimum_answered": 10, "counting": COUNTING}),
        ),
        (
            "key_fields",
            "team",
            json!({"target_precision": 0.8, "minimum_answered": 10, "typed_longest": 3}),
        ),
    ];
    for (call, attribute, options) in selections {
        let text = options.to_string();
        let select = match call {
            "layers" => gil_select_layers,
            "coarse" => gil_select_coarse,
            _ => gil_select_key_fields,
        };
        let found = result(unsafe { select(schema, s(attribute), settled, s(&text), &mut len) });
        calls.push(
            json!({"call": call, "attribute": attribute, "options": options, "result": found}),
        );
    }
    let found =
        result(unsafe { gil_select_key_concentrations(schema, s("team"), settled, &mut len) });
    calls.push(json!({"call": "key_concentrations", "attribute": "team", "result": found}));

    // A tuning: captured, given the first layers selection's promise, applied, and stale once the records grow by a
    // tenth; and that selection put on the schema.
    let captured = result(unsafe { gil_tuning_capture(schema, CHOSEN, 0.8, &mut len) });
    let thresholds = calls[0]["result"]["thresholds"].to_string();
    let promised = result(unsafe {
        gil_tuning_promise(
            s(&captured.to_string()),
            s("team"),
            s(&thresholds),
            &mut len,
        )
    });
    let mut applied = ptr::null_mut();
    assert_eq!(
        unsafe { gil_tuning_apply(s(&promised.to_string()), schema, &mut applied) },
        GIL_OK
    );
    let applied_json = result(unsafe { gil_schema_json(applied, &mut len) });
    unsafe { gil_schema_free(applied) };
    let mut with_layers = ptr::null_mut();
    assert_eq!(
        unsafe { gil_schema_with_layers(schema, s("team"), s(&thresholds), &mut with_layers) },
        GIL_OK
    );
    let with_layers_json = result(unsafe { gil_schema_json(with_layers, &mut len) });
    unsafe { gil_schema_free(with_layers) };
    let stale = result(unsafe {
        gil_tuning_stale(s(&promised.to_string()), schema, CHOSEN * 11 / 10, &mut len)
    });
    calls.push(json!({
        "call": "tuning",
        "attribute": "team",
        "settled": CHOSEN,
        "target_precision": 0.8,
        "captured": versionless(captured),
        "promised": versionless(promised),
        "applied": applied_json,
        "with_layers": with_layers_json,
        "stale_at": CHOSEN * 11 / 10,
        "stale": stale,
    }));

    // Usage counts over decisions of every shape: answered and settled, corrected, checked, entered with no answer,
    // unsettled, settled on another's answer; a promise of NaN is none.
    let given: [Given; 6] = [
        (
            Some("network"),
            Some("network"),
            Some("Accept"),
            1.0,
            Some(0.85),
        ),
        (
            Some("network"),
            Some("security"),
            Some("Correct"),
            1.0,
            Some(0.85),
        ),
        (Some("network"), Some("network"), Some("Check"), 0.25, None),
        (None, Some("facilities"), Some("Enter"), 1.0, None),
        (Some("network"), None, None, 1.0, Some(0.85)),
        (
            Some("network"),
            Some("x"),
            Some("AcceptOther"),
            1.0,
            Some(0.85),
        ),
    ];
    let flat: Vec<GilDecision> = given
        .iter()
        .map(
            |&(answer, settled, settlement, probability, promise)| GilDecision {
                attribute: s("team"),
                answer: opt(answer),
                settled: opt(settled),
                settlement: opt(settlement),
                probability,
                promise: promise.unwrap_or(f64::NAN),
            },
        )
        .collect();
    let counts = result(unsafe { gil_usage(flat.as_ptr(), flat.len(), &mut len) });
    let decisions: Vec<Value> = given
        .iter()
        .map(|&(answer, settled, settlement, probability, promise)| {
            json!({"attribute": "team", "answer": answer, "settled": settled, "settlement": settlement,
                   "probability": probability, "promise": promise})
        })
        .collect();
    calls.push(json!({"call": "usage", "decisions": decisions, "result": counts}));

    unsafe {
        gil_records_free(settled);
        gil_schema_free(schema);
    }
    check_or_write(
        "selections.json",
        &(serde_json::to_string_pretty(&calls).unwrap() + "\n"),
    );
}
