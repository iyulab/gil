//! A C ABI over the Gil decision core (`iyulab-gil`), for the .NET binding.
//!
//! State lives behind handles the caller creates and frees: a [`GilSchema`] (a schema read and checked once) and a
//! [`GilDecider`] (the memories that answer, filled once from settled records and then kept up to date). Definitions,
//! records and batch results cross as JSON in the core's `serde` form; nothing calls back into the host. A handle is not
//! thread-safe: a caller that shares one across threads holds a lock around each call.
//!
//! Every function returns a status — `GIL_OK`, or a code whose message `gil_last_error` copies out — and no panic
//! crosses the boundary. A function with a text result keeps it on the calling thread for `gil_last_result` and writes
//! its length to `result_len`. A call the core's contract rules out (asking about an attribute that is not decided, say)
//! comes back as `GIL_PANIC`; after a `GIL_PANIC` from a call that changes a handle, free that handle.

use std::collections::BTreeMap;

use gil::decider::{Answer, Decider};
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::schema::{Record, Schema};
use gil_ffi_support::{
    Failure, Utf8, argument, call, hand_out, handle, handle_mut, input, release, set_result,
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

/// UTF-8 text the caller owns, for the length of one call.
pub type GilStr = Utf8;

/// A schema, read and checked once.
pub struct GilSchema(Schema);

/// The memories that answer: the settled key memory and, unless left out, the lexical similar record memory.
pub struct GilDecider(Decider);

/// # Safety
/// As [`gil_ffi_support::text`].
unsafe fn text<'a>(s: GilStr, what: &str) -> Result<&'a str, Failure> {
    // SAFETY: forwarded from the caller.
    unsafe { gil_ffi_support::text(s, what) }
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
        let answers = decider.0.answer(&schema.0, record_id, &values, &sets);
        let value = Value::Array(answers.iter().map(answer_json).collect());
        // SAFETY: forwarded from the caller.
        unsafe { result(&value, result_len) }
    })
}
