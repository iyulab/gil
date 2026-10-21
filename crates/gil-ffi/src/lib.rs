//! A C ABI over the Gil decision core (`iyulab-gil`), for the .NET binding.
//!
//! State lives behind handles the caller creates and frees: a [`GilSchema`] (a schema read and checked once), a
//! [`GilDecider`] (the memories that answer, filled once from settled records and then kept up to date) and a
//! [`GilOpenRecord`] (one record while its values arrive). Definitions, records and batch results cross as JSON in the
//! core's `serde` form; the answer asked for while a person types (`gil_open_record_answer_partial`) comes back in flat
//! structures and buffers the caller owns, with no JSON either way. Nothing calls back into the host: an answer that
//! wants an external judgement says so, and the host offers the judgement back. The core keeps no clock — a time is
//! given in 100-nanosecond ticks since the Unix epoch, UTC. A handle is not thread-safe: a caller that shares one across
//! threads holds a lock around each call.
//!
//! Every function returns a status — `GIL_OK`, or a code whose message `gil_last_error` copies out — and no panic
//! crosses the boundary. A function with a text result keeps it on the calling thread for `gil_last_result` and writes
//! its length to `result_len`. A call the core's contract rules out (asking about an attribute that is not decided, say)
//! comes back as `GIL_PANIC`; after a `GIL_PANIC` from a call that changes a handle, free that handle.

use std::collections::BTreeMap;

use gil::decider::{Answer, Candidate, Decider, ExternalCandidate, Source};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::schema::{AttributePolicy, Record, Schema, Settlement, SettlementKind};
use gil_ffi_support::{
    BUFFER_TOO_SMALL, Failure, Utf8, argument, call, hand_out, handle, handle_mut, input, release,
    set_result, slice, slice_mut, write,
};
use serde_json::{Value, json};

/// The call succeeded.
pub const GIL_OK: i32 = gil_ffi_support::OK;
/// The core refused its input (a schema that does not check, a record that does not read); the message says what.
pub const GIL_INVALID_INPUT: i32 = gil_ffi_support::INVALID_INPUT;
/// The call itself was malformed: a null pointer, text that is not UTF-8, or a buffer too small.
pub const GIL_INVALID_ARGUMENT: i32 = gil_ffi_support::INVALID_ARGUMENT;
/// The core panicked — a call its contract rules out, or a defect — reported instead of unwinding into the caller.
pub const GIL_PANIC: i32 = gil_ffi_support::PANIC;
/// The caller's buffers are too small for the answer; the sizes it needs are in the answer header.
pub const GIL_BUFFER_TOO_SMALL: i32 = gil_ffi_support::BUFFER_TOO_SMALL;

/// [`GilCandidate::source`]: no layer.
pub const GIL_SOURCE_NONE: u8 = 0;
/// [`GilCandidate::source`]: the settled key memory.
pub const GIL_SOURCE_SETTLED_FIELD_MEMORY: u8 = 1;
/// [`GilCandidate::source`]: a similar settled record.
pub const GIL_SOURCE_SIMILAR_DOCUMENT: u8 = 2;
/// [`GilCandidate::source`]: an external judgement the host offered back (the reference's `Model`).
pub const GIL_SOURCE_MODEL: u8 = 3;

/// [`GilAnswer::policy`]: answer when a layer is confident enough.
pub const GIL_POLICY_SUGGEST: u8 = 0;
/// [`GilAnswer::policy`]: answer, but never fill on the host's own.
pub const GIL_POLICY_CONFIRM_REQUIRED: u8 = 1;
/// [`GilAnswer::policy`]: never answer.
pub const GIL_POLICY_OFF: u8 = 2;

/// UTF-8 text the caller owns, for the length of one call.
pub type GilStr = Utf8;

/// A schema, read and checked once.
pub struct GilSchema(Schema);

/// The memories that answer: the settled key memory and, unless left out, the lexical similar record memory.
pub struct GilDecider(Decider);

/// One record of a schema while its values arrive.
pub struct GilOpenRecord(OpenRecord);

/// A candidate of a flat answer: its value — `value_len` bytes of UTF-8 at `value_offset` in the call's value buffer —
/// its score on its layer's scale, its layer (`GIL_SOURCE_…`) and whether the layer trusts it. What backs it is left
/// out; the JSON answers carry it.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GilCandidate {
    pub value_offset: u32,
    pub value_len: u32,
    pub score: f64,
    pub source: u8,
    pub trusted: bool,
}

/// A flat answer for one attribute: the layer of its first candidate, how it is offered (`GIL_POLICY_…`), whether it
/// wants an external judgement (the pairs to read are in the JSON answer), whether the attribute is answered (its first
/// candidate trusted), the coarse class offered instead when there is one, and how many candidates and value bytes the
/// answer holds — written even when the caller's buffers are too small, so that it knows what to give.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GilAnswer {
    pub source: u8,
    pub policy: u8,
    pub needs_external: bool,
    pub answered: bool,
    pub has_coarse: bool,
    pub coarse: GilCandidate,
    pub candidates: u32,
    pub value_bytes: u32,
}

/// A value an external judgement ranked: its probability on the judgement's own scale and whether it trusts it.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilExternalCandidate {
    pub value: GilStr,
    pub probability: f64,
    pub trusted: bool,
}

/// # Safety
/// As [`gil_ffi_support::text`].
unsafe fn text<'a>(s: GilStr, what: &str) -> Result<&'a str, Failure> {
    // SAFETY: forwarded from the caller.
    unsafe { gil_ffi_support::text(s, what) }
}

/// Text that may be absent: a null pointer is none (with length 0), anything else is text.
///
/// # Safety
/// As [`gil_ffi_support::text`].
unsafe fn optional<'a>(s: GilStr, what: &str) -> Result<Option<&'a str>, Failure> {
    if s.ptr.is_null() && s.len == 0 {
        return Ok(None);
    }
    // SAFETY: forwarded from the caller.
    unsafe { text(s, what) }.map(Some)
}

/// Keeps `value` as this call's result and writes its length to `result_len`.
///
/// # Safety
/// `result_len` is null or points to one writable `usize`.
unsafe fn result(value: &Value, result_len: *mut usize) -> Result<(), Failure> {
    let text = value.to_string();
    let len = text.len();
    // SAFETY: forwarded from the caller.
    unsafe { gil_ffi_support::write(result_len, len, "result_len") }?;
    set_result(text);
    Ok(())
}

/// Keeps `answers` as this call's result, as a JSON array.
///
/// # Safety
/// As [`result`].
unsafe fn answers(answers: &[Answer], result_len: *mut usize) -> Result<(), Failure> {
    let value = Value::Array(answers.iter().map(answer_json).collect());
    // SAFETY: forwarded from the caller.
    unsafe { result(&value, result_len) }
}

/// An answer in the core's `serde` form, with `answered` (whether the first candidate is trusted) beside it.
fn answer_json(a: &Answer) -> Value {
    let mut value = serde_json::to_value(a).expect("an answer serializes");
    value["answered"] = json!(a.answered());
    value
}

/// Copies the message of the last refused call on this thread into `buffer` as UTF-8 (no terminator) and returns its
/// full length in bytes — call with a capacity of 0 to learn the length. Nothing is written when the message does not
/// fit. An empty message means the last call succeeded.
///
/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_last_error(buffer: *mut u8, capacity: usize) -> usize {
    // SAFETY: forwarded from the caller.
    unsafe { gil_ffi_support::copy_last_error(buffer, capacity) }
}

/// Copies the text result of the last call on this thread (the length that call wrote to its `result_len`) into
/// `buffer` as UTF-8 (no terminator) and returns its full length. Nothing is written when it does not fit; the result
/// stays until the thread's next call.
///
/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_last_result(buffer: *mut u8, capacity: usize) -> usize {
    // SAFETY: forwarded from the caller.
    unsafe { gil_ffi_support::copy_last_result(buffer, capacity) }
}

/// Reads a schema from JSON in the core's `serde` form — the settings an attribute leaves out take their defaults —
/// checks it as the reference implementation checks one, and hands it out through `out`.
///
/// # Safety
/// `json` is valid for the call; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_schema_new(json: GilStr, out: *mut *mut GilSchema) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller.
        let json = unsafe { text(json, "json") }?;
        let schema: Schema = serde_json::from_str(json)
            .map_err(|e| input(format!("a schema does not read: {e}")))?;
        schema.check().map_err(input)?;
        // SAFETY: forwarded from the caller.
        unsafe { hand_out(out, GilSchema(schema), "out") }
    })
}

/// Frees a schema; null does nothing.
///
/// # Safety
/// `schema` is null or a live schema handle, freed once and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_schema_free(schema: *mut GilSchema) {
    // SAFETY: forwarded from the caller.
    unsafe { release(schema) }
}

/// A decider with empty memories — with the similar record memory unless `similar_records` is false — handed out
/// through `out`.
///
/// # Safety
/// `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_new(similar_records: bool, out: *mut *mut GilDecider) -> i32 {
    call(|| {
        let decider = Decider::new(
            KeyMemory::default(),
            similar_records.then(LexicalMemory::default),
        );
        // SAFETY: forwarded from the caller.
        unsafe { hand_out(out, GilDecider(decider), "out") }
    })
}

/// Frees a decider; null does nothing.
///
/// # Safety
/// `decider` is null or a live decider handle, freed once and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_free(decider: *mut GilDecider) {
    // SAFETY: forwarded from the caller.
    unsafe { release(decider) }
}

/// Puts settled records of `schema` into the decider's memories, each replacing what it contributed before. `records`
/// is JSON lines, one record per line in the core's `serde` form (`settled_at` in 100-nanosecond ticks since the Unix
/// epoch, UTC), in settlement order and later than those the decider holds; blank lines are skipped. Every line is read
/// before any is put, so a line that does not read leaves the memories as they were.
///
/// # Safety
/// `decider` and `schema` are live handles; `records` is valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_put(
    decider: *mut GilDecider,
    schema: *const GilSchema,
    records: GilStr,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let decider = unsafe { handle_mut(decider, "decider") }?;
        let schema = unsafe { handle(schema, "schema") }?;
        let records = unsafe { text(records, "records") }?;
        let records: Vec<Record> = records
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(at, line)| {
                serde_json::from_str(line)
                    .map_err(|e| input(format!("record line {} does not read: {e}", at + 1)))
            })
            .collect::<Result<_, _>>()?;
        decider.0.rebuild(&schema.0, &records);
        Ok(())
    })
}

/// Answers every open decided attribute of record `record_id` — without a value, unless it takes several, and not off —
/// from the values given, writing nothing to memory; the saved version of the record, if memory holds one, is not
/// evidence for itself. `values` is a JSON object of strings; `sets` a JSON object of string lists for the attributes
/// that take several, or empty for none. The result is a JSON array of answers in the core's `serde` form, each with
/// `answered` beside it, in the schema's order.
///
/// # Safety
/// `decider` and `schema` are live handles; the texts are valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_answer(
    decider: *const GilDecider,
    schema: *const GilSchema,
    record_id: GilStr,
    values: GilStr,
    sets: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let decider = unsafe { handle(decider, "decider") }?;
        let schema = unsafe { handle(schema, "schema") }?;
        let record_id = unsafe { text(record_id, "record_id") }?;
        let values = unsafe { text(values, "values") }?;
        let sets = unsafe { text(sets, "sets") }?;
        let values: BTreeMap<String, String> = serde_json::from_str(values)
            .map_err(|e| argument(format!("values are a JSON object of strings: {e}")))?;
        let sets: BTreeMap<String, Vec<String>> = if sets.trim().is_empty() {
            BTreeMap::new()
        } else {
            serde_json::from_str(sets)
                .map_err(|e| argument(format!("sets are a JSON object of string lists: {e}")))?
        };
        let found = decider.0.answer(&schema.0, record_id, &values, &sets);
        // SAFETY: forwarded from the caller.
        unsafe { answers(&found, result_len) }
    })
}

fn source_code(source: Source) -> u8 {
    match source {
        Source::None => GIL_SOURCE_NONE,
        Source::SettledFieldMemory => GIL_SOURCE_SETTLED_FIELD_MEMORY,
        Source::SimilarDocument => GIL_SOURCE_SIMILAR_DOCUMENT,
        Source::External => GIL_SOURCE_MODEL,
    }
}

fn policy_code(policy: AttributePolicy) -> u8 {
    match policy {
        AttributePolicy::Suggest => GIL_POLICY_SUGGEST,
        AttributePolicy::ConfirmRequired => GIL_POLICY_CONFIRM_REQUIRED,
        AttributePolicy::Off => GIL_POLICY_OFF,
    }
}

/// A new open record `id` of `schema` (the record keeps its own copy), starting at `settled_at` in ticks — the present
/// for a new record — handed out through `out`.
///
/// # Safety
/// `schema` is a live handle; `id` is valid for the call; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_new(
    schema: *const GilSchema,
    id: GilStr,
    settled_at: i64,
    out: *mut *mut GilOpenRecord,
) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let schema = unsafe { handle(schema, "schema") }?;
        let id = unsafe { text(id, "id") }?;
        if id.trim().is_empty() {
            return Err(input("a record needs an id"));
        }
        let record = OpenRecord::new(schema.0.clone(), id, settled_at);
        unsafe { hand_out(out, GilOpenRecord(record), "out") }
    })
}

/// A saved record — JSON in the core's `serde` form, as `gil_open_record_snapshot` gives it — reopened as it was left,
/// handed out through `out`. Memory is not touched until a value changes.
///
/// # Safety
/// `schema` is a live handle; `record` is valid for the call; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_reopen(
    schema: *const GilSchema,
    record: GilStr,
    out: *mut *mut GilOpenRecord,
) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let schema = unsafe { handle(schema, "schema") }?;
        let record = unsafe { text(record, "record") }?;
        let saved: Record = serde_json::from_str(record)
            .map_err(|e| input(format!("a record does not read: {e}")))?;
        if saved.id.trim().is_empty() {
            return Err(input("a record needs an id"));
        }
        let record = OpenRecord::reopen(schema.0.clone(), &saved);
        unsafe { hand_out(out, GilOpenRecord(record), "out") }
    })
}

/// Frees an open record; null does nothing.
///
/// # Safety
/// `record` is null or a live open record handle, freed once and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_free(record: *mut GilOpenRecord) {
    // SAFETY: forwarded from the caller.
    unsafe { release(record) }
}

/// Sets an observed attribute's value — or clears it, given null text (null pointer, length 0) — puts the record into
/// `decider`'s memory again, and keeps as the result the fresh answers for the open decided attributes it is evidence
/// for (a JSON array, as `gil_decider_answer`'s). An attribute that is not observed is a contract violation
/// (`GIL_PANIC`).
///
/// # Safety
/// `record` and `decider` are live handles; the texts are valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_observe(
    record: *mut GilOpenRecord,
    decider: *mut GilDecider,
    attribute: GilStr,
    value: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = unsafe { handle_mut(record, "record") }?;
        let decider = unsafe { handle_mut(decider, "decider") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let value = unsafe { optional(value, "value") }?;
        let found = record.0.observe(&mut decider.0, attribute, value);
        unsafe { answers(&found, result_len) }
    })
}

/// Settles a decided attribute at `now` (ticks): `kind` by name (`Accept`, `Correct`, `Reject`, `Revert`, `Restore`,
/// `Set`, `Enter`, `Fill`, `Check`, `AcceptOther`, `CorrectOther`), with the parts it needs — `value` for the kinds that
/// settle one (null text when none), `values` for `Set` (and for `Restore` of an attribute that takes several, chosen by
/// giving `values` non-null), and `probability` for `Check` (any other kind ignores it). Puts the record into
/// `decider`'s memory again and keeps as the result the fresh answers it causes (a JSON array). A kind or part that
/// does not fit is refused (`GIL_INVALID_INPUT`); an attribute that is not decided, or a settlement of the other kind
/// of attribute, is a contract violation (`GIL_PANIC`).
///
/// # Safety
/// `record` and `decider` are live handles; the texts are valid for the call; `values` is null or holds `values_len`
/// texts; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn gil_open_record_settle(
    record: *mut GilOpenRecord,
    decider: *mut GilDecider,
    attribute: GilStr,
    kind: GilStr,
    now: i64,
    value: GilStr,
    values: *const GilStr,
    values_len: usize,
    probability: f64,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = unsafe { handle_mut(record, "record") }?;
        let decider = unsafe { handle_mut(decider, "decider") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let kind = unsafe { text(kind, "kind") }?;
        let kind = SettlementKind::parse(kind)
            .ok_or_else(|| input(format!("unknown settlement kind '{kind}'")))?;
        let value = unsafe { optional(value, "value") }?;
        let values: Option<Vec<&str>> = if values.is_null() {
            None
        } else {
            let given = unsafe { slice(values, values_len, "values") }?;
            Some(
                given
                    .iter()
                    .map(|&v| unsafe { text(v, "a value") })
                    .collect::<Result<_, _>>()?,
            )
        };
        let settlement =
            Settlement::new(kind, value, values.as_deref(), Some(probability)).map_err(input)?;
        let found = record.0.settle(&mut decider.0, attribute, &settlement, now);
        unsafe { answers(&found, result_len) }
    })
}

/// Keeps as the result the answers for every open decided attribute — not settled (unless it takes several), not
/// rejected, and not off — as a JSON array.
///
/// # Safety
/// `record` and `decider` are live handles; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_answer(
    record: *const GilOpenRecord,
    decider: *const GilDecider,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = unsafe { handle(record, "record") }?;
        let decider = unsafe { handle(decider, "decider") }?;
        let found = record.0.answer(&decider.0);
        unsafe { answers(&found, result_len) }
    })
}

/// The answer for one open decided attribute alone — with `partial` (null text when none), the text typed into it so
/// far — written flat: the header to `answer`, the candidates best first to `candidates`, and their values (and the
/// coarse class's) back to back to `value_bytes`. When `candidates_capacity` or `value_bytes_capacity` is too small,
/// only the header is written — with the counts the answer needs — and the call returns `GIL_BUFFER_TOO_SMALL`. No JSON
/// either way. An attribute that is not open (not decided, off, settled or rejected) is a contract violation
/// (`GIL_PANIC`).
///
/// # Safety
/// `record` and `decider` are live handles; the texts are valid for the call; `answer` points to one writable
/// `GilAnswer`; `candidates` holds `candidates_capacity` and `value_bytes` holds `value_bytes_capacity` writable
/// values, or is null with capacity 0.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn gil_open_record_answer_partial(
    record: *const GilOpenRecord,
    decider: *const GilDecider,
    attribute: GilStr,
    partial: GilStr,
    answer: *mut GilAnswer,
    candidates: *mut GilCandidate,
    candidates_capacity: usize,
    value_bytes: *mut u8,
    value_bytes_capacity: usize,
) -> i32 {
    call(|| {
        if answer.is_null() {
            return Err(argument("answer is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = unsafe { handle(record, "record") }?;
        let decider = unsafe { handle(decider, "decider") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let partial = unsafe { optional(partial, "partial") }?;
        let slots = unsafe { slice_mut(candidates, candidates_capacity, "candidates") }?;
        let bytes = unsafe { slice_mut(value_bytes, value_bytes_capacity, "value_bytes") }?;

        let found = record.0.answer_partial(&decider.0, attribute, partial);
        let total: usize = found
            .candidates
            .iter()
            .chain(&found.coarse)
            .map(|c| c.value.len())
            .sum();
        let too_long = || argument("an answer's values run past u32::MAX bytes");
        let mut head = GilAnswer {
            source: source_code(found.source),
            policy: policy_code(found.policy),
            needs_external: found.needs_external.is_some(),
            answered: found.answered(),
            has_coarse: found.coarse.is_some(),
            coarse: GilCandidate::default(),
            candidates: u32::try_from(found.candidates.len()).map_err(|_| too_long())?,
            value_bytes: u32::try_from(total).map_err(|_| too_long())?,
        };
        if found.candidates.len() > slots.len() || total > bytes.len() {
            unsafe { write(answer, head, "answer") }?;
            return Err(Failure(
                BUFFER_TOO_SMALL,
                format!(
                    "the answer holds {} candidates and {total} value bytes; the buffers hold {} and {}",
                    found.candidates.len(),
                    slots.len(),
                    bytes.len()
                ),
            ));
        }

        let mut at = 0usize;
        let mut flat = |c: &Candidate| {
            let start = at;
            bytes[start..start + c.value.len()].copy_from_slice(c.value.as_bytes());
            at += c.value.len();
            GilCandidate {
                // Both fit: `total` does.
                value_offset: start as u32,
                value_len: c.value.len() as u32,
                score: c.score,
                source: source_code(c.source),
                trusted: c.trusted,
            }
        };
        for (slot, c) in slots.iter_mut().zip(&found.candidates) {
            *slot = flat(c);
        }
        if let Some(coarse) = &found.coarse {
            head.coarse = flat(coarse);
        }
        unsafe { write(answer, head, "answer") }
    })
}

/// Keeps as the result the answer for an open decided attribute with an external judgement's ranked values in it —
/// what the host's model made of the evidence an earlier answer asked for — as JSON. An attribute that is not open is a
/// contract violation (`GIL_PANIC`).
///
/// # Safety
/// `record` and `decider` are live handles; `attribute` is valid for the call; `ranked` holds `ranked_len` values, or
/// is null with length 0; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_offer_external(
    record: *const GilOpenRecord,
    decider: *const GilDecider,
    attribute: GilStr,
    ranked: *const GilExternalCandidate,
    ranked_len: usize,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = unsafe { handle(record, "record") }?;
        let decider = unsafe { handle(decider, "decider") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let ranked: Vec<ExternalCandidate> = unsafe { slice(ranked, ranked_len, "ranked") }?
            .iter()
            .map(|c| {
                Ok(ExternalCandidate {
                    value: unsafe { text(c.value, "an external value") }?.to_owned(),
                    probability: c.probability,
                    trusted: c.trusted,
                })
            })
            .collect::<Result<_, Failure>>()?;
        let found = record.0.offer_external(&decider.0, attribute, &ranked);
        unsafe { result(&answer_json(&found), result_len) }
    })
}

/// Keeps as the result the record as it stands, to save — JSON in the core's `serde` form, as records are put and
/// reopened.
///
/// # Safety
/// `record` is a live handle; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_snapshot(
    record: *const GilOpenRecord,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller.
        let record = unsafe { handle(record, "record") }?;
        let value = serde_json::to_value(record.0.snapshot()).expect("a record serializes");
        unsafe { result(&value, result_len) }
    })
}
