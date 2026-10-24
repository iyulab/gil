//! Open records through the C ABI answer exactly as the core does. Each later log of the schema path's golden is
//! walked twice — once on the core's `OpenRecord`, once through the handles — from deciders filled with the first 200
//! logs: its observed values arrive one by one, each decided attribute is asked about while its value is typed (the flat
//! answer), then settled as a person would (accepting a right first candidate, correcting a wrong one). Every answer,
//! snapshot and reopened record must be the same on both sides. Also: the flat answer's buffers, an external judgement
//! offered back, refused settlements and calls the core's contract rules out — refused before anything changes when
//! the call would change a handle.

#[path = "../../gil/tests/common/mod.rs"]
mod common;

use std::ptr;

use common::{CHOSEN, fixture, lines, schema, to_record};
use gil::decider::{Answer, Decider, ExternalCandidate, Source};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::schema::{AttributeRole, Record, Schema, Settlement};
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

fn opt(text: Option<&str>) -> GilStr {
    text.map_or(NONE, s)
}

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

/// The text result of the call just made, read as JSON.
fn result(status: i32, len: usize) -> Value {
    ok(status);
    let text = copied(gil_last_result);
    assert_eq!(text.len(), len);
    serde_json::from_str(&text).unwrap()
}

fn answer_json(a: &Answer) -> Value {
    let mut value = serde_json::to_value(a).unwrap();
    value["answered"] = json!(a.answered());
    value
}

fn answers_json(answers: &[Answer]) -> Value {
    Value::Array(answers.iter().map(answer_json).collect())
}

fn source_code(source: Source) -> u8 {
    match source {
        Source::None => GIL_SOURCE_NONE,
        Source::KeyMemory => GIL_SOURCE_KEY_MEMORY,
        Source::SimilarRecord => GIL_SOURCE_SIMILAR_RECORD,
        Source::External => GIL_SOURCE_MODEL,
    }
}

/// Both sides of the walk: the core's objects and the handles over the same state.
struct Pair {
    schema: Schema,
    decider: Decider,
    schema_handle: *mut GilSchema,
    decider_handle: *mut GilDecider,
}

impl Pair {
    fn new(records: &[Record]) -> Self {
        let schema = schema(&fixture("golden.jsonl"), |_| false);
        let mut decider = Decider::new(KeyMemory::default(), Some(LexicalMemory::default()));
        decider.rebuild(&schema, records);

        let mut schema_handle = ptr::null_mut();
        let json = serde_json::to_string(&schema).unwrap();
        ok(unsafe { gil_schema_new(s(&json), &mut schema_handle) });
        let mut decider_handle = ptr::null_mut();
        ok(unsafe { gil_decider_new(true, 0, &mut decider_handle) });
        let lines: String = records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap() + "\n")
            .collect();
        let mut records_handle = ptr::null_mut();
        ok(unsafe { gil_records_new(s(&lines), &mut records_handle) });
        ok(unsafe { gil_decider_put(decider_handle, schema_handle, records_handle) });
        unsafe { gil_records_free(records_handle) };
        Self {
            schema,
            decider,
            schema_handle,
            decider_handle,
        }
    }
}

impl Drop for Pair {
    fn drop(&mut self) {
        unsafe {
            gil_decider_free(self.decider_handle);
            gil_schema_free(self.schema_handle);
        }
    }
}

fn open(pair: &Pair, id: &str, at: i64) -> (OpenRecord, *mut GilOpenRecord) {
    let mut handle = ptr::null_mut();
    ok(unsafe { gil_open_record_new(pair.schema_handle, s(id), at, NONE, &mut handle) });
    (OpenRecord::new(pair.schema.clone(), id, at), handle)
}

/// The flat answer for `attribute` through the C ABI, rebuilt as the JSON of the core's answer leaves it — the parts the
/// flat form carries.
fn flat_answer(
    record: *const GilOpenRecord,
    decider: *const GilDecider,
    attribute: &str,
    partial: Option<&str>,
) -> Value {
    let mut head = GilAnswer::default();
    let mut slots = vec![GilCandidate::default(); 8];
    let mut bytes = vec![0u8; 512];
    ok(unsafe {
        gil_open_record_answer_partial(
            record,
            decider,
            s(attribute),
            opt(partial),
            &mut head,
            slots.as_mut_ptr(),
            slots.len(),
            bytes.as_mut_ptr(),
            bytes.len(),
        )
    });
    let value = |c: &GilCandidate| {
        let start = c.value_offset as usize;
        std::str::from_utf8(&bytes[start..start + c.value_len as usize])
            .unwrap()
            .to_owned()
    };
    let candidate = |c: &GilCandidate| json!([value(c), c.score, c.source, c.trusted]);
    json!({
        "source": head.source,
        "policy": head.policy,
        "needs_external": head.needs_external,
        "answered": head.answered,
        "candidates": slots[..head.candidates as usize].iter().map(candidate).collect::<Vec<_>>(),
        "coarse": head.has_coarse.then(|| candidate(&head.coarse)),
        "value_bytes": head.value_bytes,
    })
}

fn core_flat(a: &Answer) -> Value {
    let candidate =
        |c: &gil::decider::Candidate| json!([c.value, c.score, source_code(c.source), c.trusted]);
    let policy = serde_json::to_value(a.policy).unwrap();
    let policy = match policy.as_str().unwrap() {
        "suggest" => GIL_POLICY_SUGGEST,
        "confirm_required" => GIL_POLICY_CONFIRM_REQUIRED,
        _ => GIL_POLICY_OFF,
    };
    json!({
        "source": source_code(a.source),
        "policy": policy,
        "needs_external": a.needs_external.is_some(),
        "answered": a.answered(),
        "candidates": a.candidates.iter().map(candidate).collect::<Vec<_>>(),
        "coarse": a.coarse.as_ref().map(candidate),
        "value_bytes": a.candidates.iter().chain(&a.coarse).map(|c| c.value.len()).sum::<usize>(),
    })
}

#[test]
fn open_records_through_the_c_abi_answer_as_the_core_does() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let mut pair = Pair::new(&records[..CHOSEN]);
    let mut len = 0;
    let (mut flat_checked, mut settled) = (0, 0);

    for log in records[CHOSEN..].iter().take(60) {
        let (mut core, handle) = open(&pair, &log.id, log.settled_at);
        let attributes = pair.schema.attributes();

        for a in attributes
            .iter()
            .filter(|a| a.role == AttributeRole::Observed)
        {
            let value = log.values.get(&a.name).map(String::as_str);
            let expected = answers_json(&core.observe(&mut pair.decider, &a.name, value));
            let status = unsafe {
                gil_open_record_observe(
                    handle,
                    pair.decider_handle,
                    s(&a.name),
                    opt(value),
                    &mut len,
                )
            };
            assert_eq!(
                result(status, len),
                expected,
                "{} observe {}",
                log.id,
                a.name
            );
        }

        let status =
            unsafe { gil_open_record_answer(handle, pair.decider_handle, false, &mut len) };
        assert_eq!(
            result(status, len),
            answers_json(&core.answer(&pair.decider))
        );
        let mut records = Vec::new();
        let with = core.answer_with(&pair.decider, &mut |r| records.push(r.clone()));
        let status = unsafe { gil_open_record_answer(handle, pair.decider_handle, true, &mut len) };
        let mut expected = answers_json(&with);
        for (item, record) in expected.as_array_mut().unwrap().iter_mut().zip(&records) {
            item["decision"] = serde_json::to_value(record).unwrap();
        }
        assert_eq!(
            result(status, len),
            expected,
            "{} answer with decisions",
            log.id
        );

        for a in attributes
            .iter()
            .filter(|a| a.role == AttributeRole::Decided && !a.multiple)
        {
            let Some(truth) = log.values.get(&a.name) else {
                continue;
            };
            let typed: Vec<&str> = [0, 1, 2]
                .iter()
                .filter_map(|&n| truth.char_indices().nth(n).map(|(i, _)| &truth[..i]))
                .collect();
            for partial in typed.iter().map(|&p| (!p.is_empty()).then_some(p)) {
                let expected = core_flat(&core.answer_partial(&pair.decider, &a.name, partial));
                let actual = flat_answer(handle, pair.decider_handle, &a.name, partial);
                assert_eq!(actual, expected, "{} {} typed {partial:?}", log.id, a.name);
                flat_checked += 1;
            }

            let first = core.answer_partial(&pair.decider, &a.name, None);
            let accepted = first.candidates.first().is_some_and(|c| &c.value == truth);
            let (kind, settlement) = if accepted {
                ("Accept", Settlement::accept(truth))
            } else {
                ("Correct", Settlement::correct(truth))
            };
            let expected =
                answers_json(&core.settle(&mut pair.decider, &a.name, &settlement, log.settled_at));
            let status = unsafe {
                gil_open_record_settle(
                    handle,
                    pair.decider_handle,
                    s(&a.name),
                    s(kind),
                    log.settled_at,
                    s(truth),
                    ptr::null(),
                    0,
                    1.0,
                    &mut len,
                )
            };
            assert_eq!(
                result(status, len),
                expected,
                "{} settle {}",
                log.id,
                a.name
            );
            settled += 1;
        }

        let status = unsafe { gil_open_record_snapshot(handle, &mut len) };
        let snapshot = result(status, len);
        assert_eq!(snapshot, serde_json::to_value(core.snapshot()).unwrap());

        // Reopened from its snapshot, the record answers as the core's reopened one.
        let mut reopened = ptr::null_mut();
        let text = snapshot.to_string();
        ok(unsafe { gil_open_record_reopen(pair.schema_handle, s(&text), &mut reopened) });
        let saved: Record = serde_json::from_value(snapshot).unwrap();
        let core_reopened = OpenRecord::reopen(pair.schema.clone(), &saved);
        let status =
            unsafe { gil_open_record_answer(reopened, pair.decider_handle, false, &mut len) };
        assert_eq!(
            result(status, len),
            answers_json(&core_reopened.answer(&pair.decider))
        );
        unsafe {
            gil_open_record_free(reopened);
            gil_open_record_free(handle);
        }
    }
    assert!(
        flat_checked > 200 && settled > 100,
        "{flat_checked} {settled}"
    );
}

#[test]
fn an_external_judgement_offered_back_answers_as_the_core_does() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    // Few settled records: answers that want an external judgement are common.
    let mut pair = Pair::new(&records[..5]);
    let mut len = 0;
    let mut offered = 0;
    for log in &records[5..40] {
        let (mut core, handle) = open(&pair, &log.id, log.settled_at);
        for a in pair.schema.attributes() {
            if a.role != AttributeRole::Observed {
                continue;
            }
            let value = log.values.get(&a.name).map(String::as_str);
            core.observe(&mut pair.decider, &a.name, value);
            let status = unsafe {
                gil_open_record_observe(
                    handle,
                    pair.decider_handle,
                    s(&a.name),
                    opt(value),
                    &mut len,
                )
            };
            ok(status);
        }
        for answer in core.answer(&pair.decider) {
            if answer.needs_external.is_none() {
                continue;
            }
            let Some(truth) = log.values.get(&answer.attribute) else {
                continue;
            };
            let ranked = [
                ExternalCandidate {
                    value: truth.clone(),
                    probability: 0.9,
                    trusted: true,
                },
                ExternalCandidate {
                    value: "not-in-the-domain".into(),
                    probability: 0.1,
                    trusted: false,
                },
            ];
            let expected =
                answer_json(&core.offer_external(&pair.decider, &answer.attribute, &ranked));
            let flat: Vec<GilExternalCandidate> = ranked
                .iter()
                .map(|c| GilExternalCandidate {
                    value: s(&c.value),
                    probability: c.probability,
                    trusted: c.trusted,
                })
                .collect();
            let status = unsafe {
                gil_open_record_offer_external(
                    handle,
                    pair.decider_handle,
                    s(&answer.attribute),
                    flat.as_ptr(),
                    flat.len(),
                    &mut len,
                )
            };
            assert_eq!(result(status, len), expected);
            offered += 1;
        }
        unsafe { gil_open_record_free(handle) };
    }
    assert!(offered > 0);
}

fn decided_single(pair: &Pair) -> String {
    pair.schema
        .attributes()
        .iter()
        .find(|a| a.role == AttributeRole::Decided && !a.multiple)
        .unwrap()
        .name
        .clone()
}

#[test]
fn a_flat_answer_too_large_for_the_buffers_says_what_it_needs() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let pair = Pair::new(&records[..CHOSEN]);
    let attribute = decided_single(&pair);
    let (_, handle) = open(&pair, "fresh", 0);

    let mut head = GilAnswer::default();
    let status = unsafe {
        gil_open_record_answer_partial(
            handle,
            pair.decider_handle,
            s(&attribute),
            NONE,
            &mut head,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
        )
    };
    assert!(head.candidates > 0, "an answer with candidates");
    assert_eq!(status, GIL_BUFFER_TOO_SMALL, "{}", last_error());
    assert!(last_error().contains(&format!("{} candidates", head.candidates)));

    let mut slots = vec![GilCandidate::default(); head.candidates as usize];
    let mut bytes = vec![0u8; head.value_bytes as usize];
    let needed = head;
    let status = unsafe {
        gil_open_record_answer_partial(
            handle,
            pair.decider_handle,
            s(&attribute),
            NONE,
            &mut head,
            slots.as_mut_ptr(),
            slots.len(),
            bytes.as_mut_ptr(),
            bytes.len(),
        )
    };
    ok(status);
    assert_eq!(
        (head.candidates, head.value_bytes),
        (needed.candidates, needed.value_bytes)
    );
    unsafe { gil_open_record_free(handle) };
}

#[test]
fn refused_settlements_and_contract_violations_come_back_as_statuses() {
    let records: Vec<Record> = lines(&fixture("records.jsonl"))
        .map(|r| to_record(&r))
        .collect();
    let pair = Pair::new(&records[..CHOSEN]);
    let attribute = decided_single(&pair);
    let observed = pair
        .schema
        .attributes()
        .iter()
        .find(|a| a.role == AttributeRole::Observed)
        .unwrap()
        .name
        .clone();
    let (_, handle) = open(&pair, "r", 0);
    let mut len = 0;
    let settle = |kind: &str, value: GilStr, probability: f64, len: &mut usize| unsafe {
        gil_open_record_settle(
            handle,
            pair.decider_handle,
            s(&attribute),
            s(kind),
            1,
            value,
            ptr::null(),
            0,
            probability,
            len,
        )
    };

    assert_eq!(settle("Approve", s("x"), 1.0, &mut len), GIL_INVALID_INPUT);
    assert_eq!(last_error(), "unknown settlement kind 'Approve'");
    assert_eq!(settle("Accept", NONE, 1.0, &mut len), GIL_INVALID_INPUT);
    assert_eq!(last_error(), "a Accept settlement needs a value");
    assert_eq!(settle("Check", s("x"), 0.0, &mut len), GIL_INVALID_INPUT);
    assert!(last_error().starts_with("a check is drawn"));
    ok(settle("Reject", NONE, 1.0, &mut len));

    // Observing a decided attribute, settling an observed one or one the schema lacks: refused before anything
    // changes, so the record and the decider go on working. Asking about an attribute just rejected only reads, and
    // the contract rules it out.
    let status = unsafe {
        gil_open_record_observe(handle, pair.decider_handle, s(&attribute), s("x"), &mut len)
    };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert_eq!(
        last_error(),
        format!("'{attribute}' is decided; settle it instead")
    );
    let status = unsafe {
        gil_open_record_settle(
            handle,
            pair.decider_handle,
            s(&observed),
            s("Accept"),
            1,
            s("x"),
            ptr::null(),
            0,
            1.0,
            &mut len,
        )
    };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert_eq!(
        last_error(),
        format!("'{observed}' is observed; observe it instead")
    );
    let status = unsafe {
        gil_open_record_observe(handle, pair.decider_handle, s("nowhere"), s("x"), &mut len)
    };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert!(last_error().contains("has no attribute 'nowhere'"));
    let mut head = GilAnswer::default();
    let status = unsafe {
        gil_open_record_answer_partial(
            handle,
            pair.decider_handle,
            s(&attribute),
            NONE,
            &mut head,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            0,
        )
    };
    assert_eq!(status, GIL_PANIC);
    assert!(!last_error().is_empty());

    let mut out = ptr::null_mut();
    let status = unsafe { gil_open_record_new(pair.schema_handle, s("  "), 0, NONE, &mut out) };
    assert_eq!(status, GIL_INVALID_INPUT);
    assert_eq!(last_error(), "a record needs an id");
    assert!(out.is_null());

    // Clearing an observed value is null text; empty text is a value.
    ok(unsafe {
        gil_open_record_observe(handle, pair.decider_handle, s(&observed), NONE, &mut len)
    });
    unsafe { gil_open_record_free(handle) };
}

#[test]
fn the_library_says_which_interface_it_speaks() {
    assert_eq!(gil_abi_version(), GIL_ABI_VERSION);
    assert_eq!(GIL_ABI_VERSION, 7);
}
