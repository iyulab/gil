//! Choosing thresholds through the C ABI: the layers' and the coarse level's selections reproduce the core's goldens
//! (`tests/fixtures/core/golden.jsonl`, kinds `layers` and `coarse`) from records read once into a handle; the pooled
//! rule, the key attributes' and concentrations' selections, a tuning's round trip and the usage counts come back as
//! the core's own calls give them. Options that do not read are refused; a call the core's contract rules out comes
//! back as a status with the core's message.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::ptr;

use common::{CHOSEN, COUNTED, doubles, fixture, lines, replay, schema, to_record};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::replay::{
    Choice, LayerOptions, LayerThresholds, ThresholdRule, select_key_concentrations,
    select_key_fields, select_layers,
};
use gil::schema::{Record, Schema, SettlementKind};
use gil::tuning::Tuning;
use gil::usage::{Decision, usage};
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

fn last_error() -> String {
    copied(gil_last_error)
}

fn ok(status: i32) {
    assert_eq!(status, GIL_OK, "{}", last_error());
}

fn result(status: i32, len: usize) -> Value {
    ok(status);
    let text = copied(gil_last_result);
    assert_eq!(text.len(), len);
    serde_json::from_str(&text).unwrap()
}

fn early() -> Vec<Record> {
    lines(&fixture("records.jsonl"))
        .take(CHOSEN)
        .map(|r| to_record(&r))
        .collect()
}

/// A schema handle and a records handle over the same values the core is given.
struct Handles {
    schema: *mut GilSchema,
    records: *mut GilRecords,
}

impl Handles {
    fn new(schema: &Schema, records: &[Record]) -> Self {
        let mut schema_handle = ptr::null_mut();
        let json = serde_json::to_string(schema).unwrap();
        ok(unsafe { gil_schema_new(s(&json), &mut schema_handle) });
        let lines: String = records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect();
        let mut records_handle = ptr::null_mut();
        ok(unsafe { gil_records_new(s(&lines), &mut records_handle) });
        Self {
            schema: schema_handle,
            records: records_handle,
        }
    }
}

impl Drop for Handles {
    fn drop(&mut self) {
        unsafe {
            gil_records_free(self.records);
            gil_schema_free(self.schema);
        }
    }
}

const COUNTING: &str = r#"["Enter","Accept","Correct","Check","AcceptOther","CorrectOther"]"#;

fn counted_options() -> String {
    format!(r#"{{"target_precision":0.8,"minimum_answered":10,"counting":{COUNTING}}}"#)
}

#[test]
fn the_counting_option_names_the_kinds_the_golden_counts() {
    let names: Vec<SettlementKind> = serde_json::from_str(COUNTING).unwrap();
    assert_eq!(names, COUNTED);
}

#[test]
fn the_layers_selection_reproduces_its_golden() {
    let early = early();
    let golden = fixture("golden.jsonl");
    for attribute in ["team", "code"] {
        let is_layers = |l: &Value| l["kind"] == "layers" && l["field"] == attribute;
        let expected = lines(&golden).find(is_layers).unwrap();
        let handles = Handles::new(&schema(&golden, is_layers), &early);
        let mut len = 0;
        let status = unsafe {
            gil_select_layers(
                handles.schema,
                s(attribute),
                handles.records,
                s(&counted_options()),
                &mut len,
            )
        };
        let found = result(status, len);
        assert!(found.get("decisions").is_none());
        let layers: LayerThresholds = serde_json::from_value(found["thresholds"].clone()).unwrap();
        let actual = json!({
            "kind": "layers",
            "field": attribute,
            "key": replay(&layers.key),
            "memory": replay(&layers.memory),
        });
        assert_eq!(doubles(actual), doubles(expected), "attribute {attribute}");
    }
}

#[test]
fn the_coarse_selection_reproduces_its_golden() {
    let golden = fixture("golden.jsonl");
    let is_coarse = |l: &Value| l["kind"] == "coarse" && l["field"] == "code";
    let expected = lines(&golden).find(is_coarse).unwrap();
    let handles = Handles::new(&schema(&golden, is_coarse), &early());
    let mut len = 0;
    let options = format!(
        r#"{{"target_precision":0.8,"minimum_answered":10,"counting":{COUNTING},"decisions":true}}"#
    );
    let status = unsafe {
        gil_select_coarse(
            handles.schema,
            s("code"),
            handles.records,
            s(&options),
            &mut len,
        )
    };
    let found = result(status, len);
    assert!(!found["decisions"].as_array().unwrap().is_empty());
    let coarse: LayerThresholds = serde_json::from_value(found["thresholds"].clone()).unwrap();
    let actual = json!({
        "kind": "coarse",
        "field": "code",
        "key": replay(&coarse.key),
        "memory": replay(&coarse.memory),
    });
    assert_eq!(doubles(actual), doubles(expected));
}

#[test]
fn the_other_selections_and_a_tuning_come_back_as_the_core_gives_them() {
    let early = early();
    let golden = fixture("golden.jsonl");
    let schema = schema(&golden, |_| false);
    let handles = Handles::new(&schema, &early);
    let choice = Choice {
        target_precision: 0.8,
        minimum_answered: 10,
        counting: Some(&COUNTED),
    };
    let mut len = 0;

    // The pooled rule, with the replay's decision records.
    let options = format!(
        r#"{{"target_precision":0.8,"minimum_answered":10,"counting":{COUNTING},"rule":"pooled","decisions":true}}"#
    );
    let status = unsafe {
        gil_select_layers(
            handles.schema,
            s("team"),
            handles.records,
            s(&options),
            &mut len,
        )
    };
    let pooled = result(status, len);
    let core = select_layers(
        &mut KeyMemory::default(),
        &mut LexicalMemory::default(),
        &schema,
        "team",
        &early,
        choice,
        LayerOptions {
            rule: ThresholdRule::Pooled,
            ..Default::default()
        },
    );
    assert_eq!(pooled["thresholds"], serde_json::to_value(&core).unwrap());
    assert!(!pooled["decisions"].as_array().unwrap().is_empty());

    let status = unsafe {
        gil_select_key_fields(
            handles.schema,
            s("team"),
            handles.records,
            s(&counted_options()),
            &mut len,
        )
    };
    let core_fields = select_key_fields(&schema, "team", &early, choice, 0);
    assert_eq!(
        result(status, len),
        serde_json::to_value(&core_fields).unwrap()
    );

    let status = unsafe {
        gil_select_key_concentrations(handles.schema, s("team"), handles.records, &mut len)
    };
    let core_concentrations =
        select_key_concentrations(&mut KeyMemory::default(), &schema, "team", &early);
    assert_eq!(result(status, len), json!(core_concentrations));

    // A tuning: captured, given the pooled replay's promise, applied, and stale once the records grow by a tenth.
    let status = unsafe { gil_tuning_capture(handles.schema, early.len(), 0.8, &mut len) };
    let captured = result(status, len);
    let mut core_tuning = Tuning::capture(&schema, early.len(), 0.8);
    assert_eq!(captured, serde_json::to_value(&core_tuning).unwrap());

    let thresholds = pooled["thresholds"].to_string();
    let status = unsafe {
        gil_tuning_promise(
            s(&captured.to_string()),
            s("team"),
            s(&thresholds),
            &mut len,
        )
    };
    let promised = result(status, len);
    core_tuning.promise("team", &core);
    assert_eq!(promised, serde_json::to_value(&core_tuning).unwrap());

    let mut applied = ptr::null_mut();
    ok(unsafe { gil_tuning_apply(s(&promised.to_string()), handles.schema, &mut applied) });
    let core_applied = core_tuning.apply(&schema).expect("a valid schema");
    let status = unsafe { gil_schema_json(applied, &mut len) };
    assert_eq!(
        result(status, len),
        serde_json::to_value(&core_applied).unwrap()
    );

    // The pooled choice put on the schema, as the core puts it; an attribute with no coarse level refuses a coarse one.
    let mut with_layers = ptr::null_mut();
    ok(unsafe {
        gil_schema_with_layers(handles.schema, s("team"), s(&thresholds), &mut with_layers)
    });
    let status = unsafe { gil_schema_json(with_layers, &mut len) };
    assert_eq!(
        result(status, len),
        serde_json::to_value(schema.with_layers("team", &core).expect("a valid schema")).unwrap()
    );
    unsafe { gil_schema_free(with_layers) };
    let mut refused = ptr::null_mut();
    let status =
        unsafe { gil_schema_with_coarse(handles.schema, s("team"), s(&thresholds), &mut refused) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(refused.is_null());
    assert!(last_error().contains("no coarse level"), "{}", last_error());

    let stale = |settled: usize, len: &mut usize| {
        let status =
            unsafe { gil_tuning_stale(s(&promised.to_string()), handles.schema, settled, len) };
        result(status, *len)
    };
    assert_eq!(stale(early.len(), &mut len), Value::Null);
    assert_eq!(
        stale(early.len() * 11 / 10, &mut len),
        serde_json::to_value(core_tuning.stale(&schema, early.len() * 11 / 10)).unwrap()
    );
    unsafe { gil_schema_free(applied) };
}

#[test]
fn the_usage_counts_come_back_as_the_core_gives_them() {
    let flat = |answer: Option<&'static str>,
                settled: Option<&'static str>,
                settlement: Option<&'static str>,
                probability: f64,
                promise: f64| GilDecision {
        attribute: s("team"),
        answer: answer.map_or(NONE, s),
        settled: settled.map_or(NONE, s),
        settlement: settlement.map_or(NONE, s),
        probability,
        promise,
    };
    let given = [
        flat(Some("network"), Some("network"), Some("Accept"), 1.0, 0.85),
        flat(
            Some("network"),
            Some("security"),
            Some("Correct"),
            1.0,
            0.85,
        ),
        flat(
            Some("network"),
            Some("network"),
            Some("Check"),
            0.25,
            f64::NAN,
        ),
        flat(None, Some("facilities"), Some("Enter"), 1.0, f64::NAN),
        flat(Some("network"), None, None, 1.0, 0.85),
        flat(Some("network"), Some("x"), Some("AcceptOther"), 1.0, 0.85),
    ];
    let mut len = 0;
    let status = unsafe { gil_usage(given.as_ptr(), given.len(), &mut len) };
    let d = |answer, settled, settlement, probability, promise| Decision {
        attribute: "team",
        answer,
        settled,
        settlement,
        probability,
        promise,
    };
    let core = usage([
        d(
            Some("network"),
            Some("network"),
            Some(SettlementKind::Accept),
            1.0,
            Some(0.85),
        ),
        d(
            Some("network"),
            Some("security"),
            Some(SettlementKind::Correct),
            1.0,
            Some(0.85),
        ),
        d(
            Some("network"),
            Some("network"),
            Some(SettlementKind::Check),
            0.25,
            None,
        ),
        d(
            None,
            Some("facilities"),
            Some(SettlementKind::Enter),
            1.0,
            None,
        ),
        d(Some("network"), None, None, 1.0, Some(0.85)),
        d(
            Some("network"),
            Some("x"),
            Some(SettlementKind::AcceptOther),
            1.0,
            Some(0.85),
        ),
    ]);
    assert_eq!(result(status, len), json!(core));

    let bad = [flat(None, None, Some("Approve"), 1.0, f64::NAN)];
    let status = unsafe { gil_usage(bad.as_ptr(), bad.len(), &mut len) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert_eq!(last_error(), "unknown settlement kind 'Approve'");
}

#[test]
fn options_that_do_not_read_are_refused_and_a_broken_contract_is_a_status() {
    let handles = Handles::new(&schema(&fixture("golden.jsonl"), |_| false), &early());
    let mut len = 0;
    let select = |attribute: &str, options: &str, len: &mut usize| unsafe {
        gil_select_layers(
            handles.schema,
            s(attribute),
            handles.records,
            s(options),
            len,
        )
    };

    assert_eq!(select("team", "{}", &mut len), GIL_INVALID_INPUT);
    assert!(
        last_error().starts_with("the options do not read"),
        "{}",
        last_error()
    );
    let unknown = r#"{"target_precision":0.8,"minimum_answered":10,"rule":"strict"}"#;
    assert_eq!(select("team", unknown, &mut len), GIL_INVALID_INPUT);
    let extra = r#"{"target_precision":0.8,"minimum_answered":10,"precision":1}"#;
    assert_eq!(select("team", extra, &mut len), GIL_INVALID_INPUT);

    let out_of_range = r#"{"target_precision":1.5,"minimum_answered":10}"#;
    assert_eq!(select("team", out_of_range, &mut len), GIL_PANIC);
    assert_eq!(last_error(), "target_precision must be in (0, 1]");
    let observed = r#"{"target_precision":0.8,"minimum_answered":10}"#;
    assert_eq!(select("product", observed, &mut len), GIL_PANIC);
    assert_eq!(last_error(), "'product' is not a decided attribute");
    assert_eq!(copied(gil_last_result), "");
}
