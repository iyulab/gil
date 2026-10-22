//! The ask planner through the C ABI reproduces its golden (`tests/fixtures/core/ask.jsonl`) as the core does: a planner
//! per decided single-valued attribute, advice on every later record with some askable attributes left open before the
//! record is put, learning from it after. Also the refusals: a rule `new` checks, and an answer for another attribute.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::ptr;

use common::{CHOSEN, doubles, fixture, lines, schema, to_record};
use gil::schema::{AttributeRole, Record};
use gil_native::*;
use serde_json::{Value, json};

const ASKABLE: [&str; 3] = ["product", "channel", "summary"];

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

fn records_of(records: &[Record]) -> *mut GilRecords {
    let lines: Vec<String> = records
        .iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();
    let joined = lines.join("\n");
    let mut out = ptr::null_mut();
    ok(unsafe { gil_records_new(s(&joined), &mut out) });
    out
}

fn open(attribute: &str, i: usize) -> bool {
    (attribute == "product" && i.is_multiple_of(2))
        || (attribute == "channel" && i % 3 == 1)
        || (attribute == "summary" && i.is_multiple_of(5))
}

fn planner(
    schema: *const GilSchema,
    attribute: &str,
    askable: &[&str],
    budget: f64,
) -> (i32, *mut GilAskPlanner) {
    let texts: Vec<GilStr> = askable.iter().map(|a| s(a)).collect();
    let mut out = ptr::null_mut();
    let status = unsafe {
        gil_ask_planner_new(
            schema,
            s(attribute),
            texts.as_ptr(),
            texts.len(),
            budget,
            &mut out,
        )
    };
    (status, out)
}

#[test]
fn the_ask_planner_reproduces_its_golden_through_the_c_abi() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let core = schema(&fixture("golden.jsonl"), |_| false);
    let expected: Vec<Value> = lines(&fixture("ask.jsonl")).collect();
    let schema_json = serde_json::to_string(&core).unwrap();
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&schema_json), &mut schema) });
    let mut decider = ptr::null_mut();
    ok(unsafe { gil_decider_new(true, 0, &mut decider) });
    let early = records_of(&records[..CHOSEN]);
    ok(unsafe { gil_decider_put(decider, schema, early) });
    unsafe { gil_records_free(early) };
    let planners: Vec<(String, *mut GilAskPlanner)> = core
        .attributes()
        .iter()
        .filter(|a| a.role == AttributeRole::Decided && !a.multiple)
        .map(|a| {
            let (status, made) = planner(schema, &a.name, &ASKABLE, 0.3);
            ok(status);
            (a.name.clone(), made)
        })
        .collect();

    let mut at = 0;
    for (i, record) in records[CHOSEN..].iter().enumerate() {
        let values: BTreeMap<String, String> = record
            .values
            .iter()
            .filter(|(a, _)| core.attribute(a).role == AttributeRole::Observed && !open(a, i))
            .map(|(a, v)| (a.clone(), v.clone()))
            .collect();
        let values_json = serde_json::to_string(&values).unwrap();
        let mut len = 0;
        let answers = result(unsafe {
            gil_decider_answer(
                decider,
                schema,
                s(&record.id),
                s(&values_json),
                s(""),
                false,
                &mut len,
            )
        });
        for (attribute, made) in &planners {
            let answer = answers
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["attribute"] == attribute.as_str())
                .expect("every decided attribute is answered");
            let answer_json = answer.to_string();
            let advice = result(unsafe {
                gil_ask_planner_advise(*made, s(&answer_json), s(&values_json), &mut len)
            });
            let actual = json!({
                "kind": "ask",
                "doc": record.id,
                "field": attribute,
                "answered": answer["answered"],
                "coarse": answer["coarse"].as_object().map(|c| json!([c["value"], c["trusted"]])),
                "advice": advice.as_array().unwrap().iter()
                    .map(|a| json!([a["attribute"], a["gain"], a["ask"], a["threshold"]]))
                    .collect::<Vec<_>>(),
            });
            assert_eq!(
                doubles(actual),
                doubles(expected[at].clone()),
                "ask line {}",
                at + 1
            );
            at += 1;
        }

        let settled = records_of(std::slice::from_ref(record));
        ok(unsafe { gil_decider_put(decider, schema, settled) });
        for (_, made) in &planners {
            ok(unsafe { gil_ask_planner_learn(*made, decider, settled) });
        }
        unsafe { gil_records_free(settled) };
    }

    assert_eq!(at, expected.len());
    for (_, made) in planners {
        unsafe { gil_ask_planner_free(made) };
    }
    unsafe {
        gil_decider_free(decider);
        gil_schema_free(schema);
    }
}

#[test]
fn a_planner_refuses_what_the_core_refuses() {
    let core = schema(&fixture("golden.jsonl"), |_| false);
    let schema_json = serde_json::to_string(&core).unwrap();
    let mut schema = ptr::null_mut();
    ok(unsafe { gil_schema_new(s(&schema_json), &mut schema) });

    let (status, _) = planner(schema, "team", &["product"], 0.0);
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(copied(gil_last_error).contains("budget"));
    let (status, _) = planner(schema, "team", &["code"], 0.3);
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(copied(gil_last_error).contains("observed"));

    let (status, team) = planner(schema, "team", &["product"], 0.3);
    ok(status);
    let other = json!({"attribute": "code", "candidates": [], "source": "None", "coarse": null, "policy": "suggest",
        "needs_external": null})
    .to_string();
    let mut len = 0;
    let status = unsafe { gil_ask_planner_advise(team, s(&other), s("{}"), &mut len) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert_eq!(
        copied(gil_last_error),
        "the answer is for 'code', not 'team'"
    );
    unsafe {
        gil_ask_planner_free(team);
        gil_schema_free(schema);
    }
}
