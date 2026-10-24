//! A C ABI over the Gil decision core (`iyulab-gil`), for the .NET binding.
//!
//! State lives behind handles the caller creates and frees: a [`GilSchema`] (a schema read and checked once),
//! [`GilRecords`] (settled records read once — what fills a decider and what a replay chooses thresholds on), a
//! [`GilDecider`] (the memories that answer, kept up to date as records are settled), a [`GilOpenRecord`] (one record
//! while its values arrive) and a [`GilAskPlanner`] (which observed attribute to ask a person for first). Definitions, records and batch results cross as JSON in the
//! core's `serde` form; the answer asked for while a person types (`gil_open_record_answer_partial`) comes back in flat
//! structures and buffers the caller owns, with no JSON either way. Nothing calls back into the host: an answer that
//! wants an external judgement says so, and the host offers the judgement back. The core keeps no clock — a time is
//! given in 100-nanosecond ticks since the Unix epoch, UTC.
//!
//! Threads: a function that takes a handle as `*const` only reads it, and may run on several threads at once for the
//! same handle; one that takes it as `*mut` changes it, and needs it to itself — a caller that shares a handle across
//! threads holds a reader–writer lock, shared for the first kind and exclusive for the second.
//!
//! Every function returns a status — `GIL_OK`, or a code whose message `gil_last_error` copies out — and no panic
//! crosses the boundary. A function with a text result keeps it on the calling thread for `gil_last_result` and writes
//! its length to `result_len`. A call the core's contract rules out comes back as `GIL_INVALID_INPUT` when the call
//! changes a handle — it is refused before anything changes — and as `GIL_PANIC` when it only reads one (asking about an
//! attribute that is not decided, say). A `GIL_PANIC` from a call that changes a handle is a defect that may have stopped
//! the change halfway: the decider, record or planner it was changing is broken, and every later call with it returns
//! `GIL_BROKEN` until it is freed. `gil_abi_version` says which version of this interface the library speaks.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use gil::ask_planner::AskPlanner;
use gil::decider::{Answer, Candidate, Decider, ExternalCandidate, Source};
use gil::decision_record::DecisionRecord;
use gil::key_memory::KeyMemory;
use gil::lexical_memory::LexicalMemory;
use gil::open_record::OpenRecord;
use gil::replay::{
    Choice, LayerOptions, LayerThresholds, ThresholdRule, select_coarse_with,
    select_key_concentrations, select_key_fields, select_layers,
};
use gil::schema::{AttributePolicy, InView, Record, Schema, Settlement, SettlementKind};
use gil::tuning::Tuning;
use gil::usage::usage;
use iyulab_ffi_support::{
    BUFFER_TOO_SMALL, Failure, Utf8, argument, call, hand_out, handle, handle_mut, input, release,
    set_result, slice, slice_mut, write,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// The call succeeded.
pub const GIL_OK: i32 = iyulab_ffi_support::OK;
/// The core refused its input (a schema that does not check, a record that does not read); the message says what.
pub const GIL_INVALID_INPUT: i32 = iyulab_ffi_support::INVALID_INPUT;
/// The call itself was malformed: a null pointer, text that is not UTF-8, or a buffer too small.
pub const GIL_INVALID_ARGUMENT: i32 = iyulab_ffi_support::INVALID_ARGUMENT;
/// The core panicked — a call its contract rules out, or a defect — reported instead of unwinding into the caller.
pub const GIL_PANIC: i32 = iyulab_ffi_support::PANIC;
/// The caller's buffers are too small for the answer; the sizes it needs are in the answer header.
pub const GIL_BUFFER_TOO_SMALL: i32 = iyulab_ffi_support::BUFFER_TOO_SMALL;
/// A decider or record is broken: an earlier call that changed it panicked partway. Free it and build it again.
pub const GIL_BROKEN: i32 = iyulab_ffi_support::BROKEN;

/// The version of this interface — its functions, structures and statuses. It changes whenever a binding built for one
/// version would misread the library of another.
pub const GIL_ABI_VERSION: u32 = 4;

/// [`GilCandidate::source`]: no layer.
pub const GIL_SOURCE_NONE: u8 = 0;
/// [`GilCandidate::source`]: the settled key memory.
pub const GIL_SOURCE_KEY_MEMORY: u8 = 1;
/// [`GilCandidate::source`]: a similar settled record.
pub const GIL_SOURCE_SIMILAR_RECORD: u8 = 2;
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

/// Settled records of a schema, in settlement order, read once.
pub struct GilRecords(Vec<Record>);

/// The memories that answer: the settled key memory and, unless left out, the lexical similar record memory.
pub struct GilDecider {
    decider: Decider,
    broken: AtomicBool,
}

/// One record of a schema while its values arrive.
pub struct GilOpenRecord {
    record: OpenRecord,
    broken: AtomicBool,
}

/// Which observed attribute to ask a person for first so that a decided attribute gets answered, learned from settled
/// records — the planner keeps its own copy of the schema it plans for.
pub struct GilAskPlanner {
    planner: AskPlanner,
    broken: AtomicBool,
}

/// The similar record memory on its own — character n-gram TF-IDF, one index per task — for a host that keeps a memory
/// of answers outside a decider (the one a decider holds is not reached through this).
pub struct GilLexicalMemory {
    memory: LexicalMemory,
    broken: AtomicBool,
}

// What the functions that only read a handle may share across threads (see the module's threads paragraph).
const _: () = {
    const fn shared<T: Sync>() {}
    shared::<GilSchema>();
    shared::<GilRecords>();
    shared::<GilDecider>();
    shared::<GilOpenRecord>();
    shared::<GilAskPlanner>();
    shared::<GilLexicalMemory>();
};

/// A handle that a defect can break: what it says of itself, and its mark.
trait Breakable {
    const WHAT: &'static str;
    fn broken(&self) -> &AtomicBool;
}

impl Breakable for GilDecider {
    const WHAT: &'static str = "decider";
    fn broken(&self) -> &AtomicBool {
        &self.broken
    }
}

impl Breakable for GilOpenRecord {
    const WHAT: &'static str = "record";
    fn broken(&self) -> &AtomicBool {
        &self.broken
    }
}

impl Breakable for GilAskPlanner {
    const WHAT: &'static str = "planner";
    fn broken(&self) -> &AtomicBool {
        &self.broken
    }
}

impl Breakable for GilLexicalMemory {
    const WHAT: &'static str = "lexical memory";
    fn broken(&self) -> &AtomicBool {
        &self.broken
    }
}

fn refuse_broken<T: Breakable>(h: &T) -> Result<(), Failure> {
    if h.broken().load(Ordering::Relaxed) {
        return Err(Failure(
            GIL_BROKEN,
            format!(
                "the {} is broken: an earlier call that changed it panicked partway; free it and build it again",
                T::WHAT
            ),
        ));
    }
    Ok(())
}

/// The live object behind a handle, refused when broken.
///
/// # Safety
/// As [`iyulab_ffi_support::handle`].
unsafe fn live<'a, T: Breakable>(h: *const T) -> Result<&'a T, Failure> {
    // SAFETY: forwarded from the caller.
    let found = unsafe { handle(h, T::WHAT) }?;
    refuse_broken(found)?;
    Ok(found)
}

/// As [`live`], exclusively.
///
/// # Safety
/// As [`iyulab_ffi_support::handle_mut`].
unsafe fn live_mut<'a, T: Breakable>(h: *mut T) -> Result<&'a mut T, Failure> {
    // SAFETY: forwarded from the caller.
    let found = unsafe { handle_mut(h, T::WHAT) }?;
    refuse_broken(found)?;
    Ok(found)
}

/// Marks the handle a changing call was given broken when the call panicked.
///
/// # Safety
/// `h` is null or a live handle that no other thread uses during the call.
unsafe fn break_on_panic<T: Breakable>(status: i32, h: *const T) {
    if status == GIL_PANIC
        // SAFETY: forwarded from the caller.
        && let Some(found) = unsafe { h.as_ref() }
    {
        found.broken().store(true, Ordering::Relaxed);
    }
}

/// The version of this interface the library speaks ([`GIL_ABI_VERSION`]) — what a binding checks before its first
/// call.
#[unsafe(no_mangle)]
pub extern "C" fn gil_abi_version() -> u32 {
    GIL_ABI_VERSION
}

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
/// As [`iyulab_ffi_support::text`].
unsafe fn text<'a>(s: GilStr, what: &str) -> Result<&'a str, Failure> {
    // SAFETY: forwarded from the caller.
    unsafe { iyulab_ffi_support::text(s, what) }
}

/// Text that may be absent: a null pointer is none (with length 0), anything else is text.
///
/// # Safety
/// As [`iyulab_ffi_support::text`].
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
    unsafe { iyulab_ffi_support::write(result_len, len, "result_len") }?;
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

/// Answers as a JSON array, each with `decision` — its lookup's decision record, in the same order — when kept.
fn with_decisions(found: &[Answer], kept: Option<Vec<Value>>) -> Value {
    let mut value = Value::Array(found.iter().map(answer_json).collect());
    if let (Some(kept), Value::Array(items)) = (kept, &mut value) {
        for (item, record) in items.iter_mut().zip(kept) {
            item["decision"] = record;
        }
    }
    value
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
    unsafe { iyulab_ffi_support::copy_last_error(buffer, capacity) }
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
    unsafe { iyulab_ffi_support::copy_last_result(buffer, capacity) }
}

/// Reads a schema from JSON in the core's `serde` form — the settings an attribute leaves out take their defaults — and
/// hands it out through `out`. Reading checks it as `Schema::new` checks one; a schema that breaks a rule
/// does not read, and the error names the rule.
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

/// A similar record memory whose lookups are split among at most `threads` threads — 0 for the core's default.
fn record_memory(memory: LexicalMemory, threads: usize) -> LexicalMemory {
    if threads == 0 {
        memory
    } else {
        memory.with_threads(threads)
    }
}

/// A decider with empty memories — with the similar record memory unless `similar_records` is false, its lookups split
/// among at most `threads` threads (0: the core's default, 1: none) — handed out through `out`.
///
/// # Safety
/// `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_new(
    similar_records: bool,
    threads: usize,
    out: *mut *mut GilDecider,
) -> i32 {
    call(|| {
        let decider = Decider::new(
            KeyMemory::default(),
            similar_records.then(|| record_memory(LexicalMemory::default(), threads)),
        );
        // SAFETY: forwarded from the caller.
        unsafe {
            hand_out(
                out,
                GilDecider {
                    decider,
                    broken: AtomicBool::new(false),
                },
                "out",
            )
        }
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

/// Reads settled records from JSON lines — one record per line in the core's `serde` form (`settled_at` in ticks or as a
/// time), in settlement order; blank lines are skipped — and hands them out through `out`. A line that does not read
/// refuses the whole text.
///
/// # Safety
/// `lines` is valid for the call; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_records_new(lines: GilStr, out: *mut *mut GilRecords) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller.
        let lines = unsafe { text(lines, "lines") }?;
        let records: Vec<Record> = lines
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(at, line)| {
                serde_json::from_str(line)
                    .map_err(|e| input(format!("record line {} does not read: {e}", at + 1)))
            })
            .collect::<Result<_, _>>()?;
        // SAFETY: forwarded from the caller.
        unsafe { hand_out(out, GilRecords(records), "out") }
    })
}

/// How many records `records` holds; 0 for null.
///
/// # Safety
/// `records` is null or a live records handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_records_len(records: *const GilRecords) -> usize {
    // SAFETY: forwarded from the caller.
    unsafe { records.as_ref() }.map_or(0, |r| r.0.len())
}

/// Frees records; null does nothing.
///
/// # Safety
/// `records` is null or a live records handle, freed once and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_records_free(records: *mut GilRecords) {
    // SAFETY: forwarded from the caller.
    unsafe { release(records) }
}

/// Puts settled records of `schema` into the decider's memories, each replacing what it contributed before — records
/// settled later than those the decider holds.
///
/// # Safety
/// `decider`, `schema` and `records` are live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_put(
    decider: *mut GilDecider,
    schema: *const GilSchema,
    records: *const GilRecords,
) -> i32 {
    let status = call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let decider = &mut unsafe { live_mut(decider) }?.decider;
        let schema = unsafe { handle(schema, "schema") }?;
        let records = unsafe { handle(records, "records") }?;
        placed(&schema.0, &records.0)?;
        decider.rebuild(&schema.0, &records.0);
        Ok(())
    });
    // SAFETY: forwarded from the caller.
    unsafe { break_on_panic(status, decider) };
    status
}

/// Answers every open decided attribute of record `record_id` — without a value, unless it takes several, and not off —
/// from the values given, writing nothing to memory; the saved version of the record, if memory holds one, is not
/// evidence for itself. `values` is a JSON object of strings; `sets` a JSON object of string lists for the attributes
/// that take several, or empty for none. The result is a JSON array of answers in the core's `serde` form, each with
/// `answered` beside it, in the schema's order — and, when `decisions` is true, `decision`: the lookup's decision record
/// (what the key layer and the similar record layer found whatever the thresholds, and what was answered), for a host
/// that keeps them to recount its thresholds later.
///
/// # Safety
/// `decider` and `schema` are live handles; the texts are valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn gil_decider_answer(
    decider: *const GilDecider,
    schema: *const GilSchema,
    record_id: GilStr,
    values: GilStr,
    sets: GilStr,
    decisions: bool,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let decider = &unsafe { live(decider) }?.decider;
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
        schema
            .0
            .check_placement(&values, &sets)
            .map_err(|m| input(format!("record '{record_id}': {m}")))?;
        let mut kept = decisions.then(Vec::new);
        let found = decider.answer_with(&schema.0, record_id, &values, &sets, &mut |r| {
            keep(&mut kept, r)
        });
        // SAFETY: forwarded from the caller.
        unsafe { result(&with_decisions(&found, kept), result_len) }
    })
}

/// Takes record `record_id` out of the decider's memories — a record the host deleted; nothing happens when they do
/// not hold it. The key memory is then as if the record had never been put, and the similar record memory never offers
/// it again.
///
/// # Safety
/// `decider` and `schema` are live handles; `record_id` is valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_decider_remove(
    decider: *mut GilDecider,
    schema: *const GilSchema,
    record_id: GilStr,
) -> i32 {
    let status = call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let memories = &mut unsafe { live_mut(decider) }?.decider;
        let schema = unsafe { handle(schema, "schema") }?;
        let record_id = unsafe { text(record_id, "record_id") }?;
        memories.remove(&schema.0, record_id);
        Ok(())
    });
    // SAFETY: forwarded from the caller.
    unsafe { break_on_panic(status, decider) };
    status
}

fn source_code(source: Source) -> u8 {
    match source {
        Source::None => GIL_SOURCE_NONE,
        Source::KeyMemory => GIL_SOURCE_KEY_MEMORY,
        Source::SimilarRecord => GIL_SOURCE_SIMILAR_RECORD,
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
        unsafe {
            hand_out(
                out,
                GilOpenRecord {
                    record,
                    broken: AtomicBool::new(false),
                },
                "out",
            )
        }
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
        placed(&schema.0, [&saved])?;
        let record = OpenRecord::reopen(schema.0.clone(), &saved);
        unsafe {
            hand_out(
                out,
                GilOpenRecord {
                    record,
                    broken: AtomicBool::new(false),
                },
                "out",
            )
        }
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
/// for (a JSON array, as `gil_decider_answer`'s). An attribute that is not observed is refused (`GIL_INVALID_INPUT`)
/// before anything changes.
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
    let status = call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let open = &mut unsafe { live_mut(record) }?.record;
        let memories = &mut unsafe { live_mut(decider) }?.decider;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let value = unsafe { optional(value, "value") }?;
        open.check_observe(attribute)?;
        let found = open.observe(memories, attribute, value);
        unsafe { answers(&found, result_len) }
    });
    // SAFETY: forwarded from the caller, for both.
    unsafe {
        break_on_panic(status, record);
        break_on_panic(status, decider);
    }
    status
}

/// Settles a decided attribute at `now` (ticks): `kind` by name (`Accept`, `Correct`, `Reject`, `Revert`, `Restore`,
/// `Set`, `Enter`, `Fill`, `Check`, `AcceptOther`, `CorrectOther`), with the parts it needs — `value` for the kinds that
/// settle one (null text when none), `values` for `Set` (and for `Restore` of an attribute that takes several, chosen by
/// giving `values` non-null), and `probability` for `Check` (any other kind ignores it). `in_view` is the value
/// suggested when it was settled (null text when nothing was in view), `in_view_source` the layer of this decider's
/// answer it was, by name (`KeyMemory`, `SimilarRecord`, `Model`; null text for a suggestion that was not this
/// decider's) and `in_view_promise` the precision promised for it (NaN when none) — what the record keeps. Puts the record into
/// `decider`'s memory again and keeps as the result the fresh answers it causes (a JSON array). A kind or part that
/// does not fit, an attribute that is not decided, and a settlement of the other kind of attribute are refused
/// (`GIL_INVALID_INPUT`) before anything changes.
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
    in_view: GilStr,
    in_view_source: GilStr,
    in_view_promise: f64,
    result_len: *mut usize,
) -> i32 {
    let status = call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let open = &mut unsafe { live_mut(record) }?.record;
        let memories = &mut unsafe { live_mut(decider) }?.decider;
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
        let mut settlement =
            Settlement::new(kind, value, values.as_deref(), Some(probability)).map_err(input)?;
        if let Some(suggested) = unsafe { optional(in_view, "in_view") }? {
            let source = unsafe { optional(in_view_source, "in_view_source") }?
                .map(|name| {
                    [Source::KeyMemory, Source::SimilarRecord, Source::External]
                        .into_iter()
                        .find(|s| s.name() == name)
                        .ok_or_else(|| input(format!("unknown layer '{name}'")))
                })
                .transpose()?;
            settlement.in_view = Some(InView {
                value: suggested.to_owned(),
                source,
                promise: (!in_view_promise.is_nan()).then_some(in_view_promise),
            });
        }
        open.check_settle(attribute, &settlement)?;
        let found = open.settle(memories, attribute, &settlement, now);
        unsafe { answers(&found, result_len) }
    });
    // SAFETY: forwarded from the caller, for both.
    unsafe {
        break_on_panic(status, record);
        break_on_panic(status, decider);
    }
    status
}

/// Keeps as the result the answers for every open decided attribute — not settled (unless it takes several), not
/// rejected, and not off — as a JSON array, as `gil_decider_answer`'s: each with `decision`, the lookup's decision
/// record, when `decisions` is true.
///
/// # Safety
/// `record` and `decider` are live handles; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_open_record_answer(
    record: *const GilOpenRecord,
    decider: *const GilDecider,
    decisions: bool,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let record = &unsafe { live(record) }?.record;
        let decider = &unsafe { live(decider) }?.decider;
        let mut kept = decisions.then(Vec::new);
        let found = record.answer_with(decider, &mut |r| keep(&mut kept, r));
        unsafe { result(&with_decisions(&found, kept), result_len) }
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
        let record = &unsafe { live(record) }?.record;
        let decider = &unsafe { live(decider) }?.decider;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let partial = unsafe { optional(partial, "partial") }?;
        let slots = unsafe { slice_mut(candidates, candidates_capacity, "candidates") }?;
        let bytes = unsafe { slice_mut(value_bytes, value_bytes_capacity, "value_bytes") }?;

        let found = record.answer_partial(decider, attribute, partial);
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
        let record = &unsafe { live(record) }?.record;
        let decider = &unsafe { live(decider) }?.decider;
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
        let found = record.offer_external(decider, attribute, &ranked);
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
        let record = &unsafe { live(record) }?.record;
        let value = serde_json::to_value(record.snapshot()).expect("a record serializes");
        unsafe { result(&value, result_len) }
    })
}

/// What a threshold is chosen for, as the selections read it from JSON: `target_precision` in (0, 1] and
/// `minimum_answered` at least 1, and optionally `counting` (the settlement kinds whose lookups count, by name; all when
/// left out), `rule` (`every_band`, the default, or `pooled` — the layers' selection only), `decisions` (whether to keep
/// the decision records of the replay — the layers' and the coarse level's selections) and `typed_longest` (the key
/// attributes' selection: count typing up to that many characters).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    target_precision: f64,
    minimum_answered: usize,
    #[serde(default)]
    counting: Option<Vec<SettlementKind>>,
    #[serde(default)]
    rule: ThresholdRule,
    #[serde(default)]
    decisions: bool,
    #[serde(default)]
    typed_longest: usize,
    /// The most threads a similar-record lookup of the replay is split among; 0 for the core's default.
    #[serde(default)]
    threads: usize,
}

impl Options {
    fn choice(&self) -> Choice<'_> {
        Choice {
            target_precision: self.target_precision,
            minimum_answered: self.minimum_answered,
            counting: self.counting.as_deref(),
        }
    }
}

/// # Safety
/// As [`iyulab_ffi_support::text`].
unsafe fn read_options(json: GilStr) -> Result<Options, Failure> {
    // SAFETY: forwarded from the caller.
    let json = unsafe { text(json, "options") }?;
    serde_json::from_str(json).map_err(|e| input(format!("the options do not read: {e}")))
}

/// The handles and texts every selection takes.
///
/// # Safety
/// As the selections'.
unsafe fn selection<'a>(
    schema: *const GilSchema,
    attribute: GilStr,
    records: *const GilRecords,
) -> Result<(&'a Schema, &'a str, &'a [Record]), Failure> {
    // SAFETY: forwarded from the caller, for every pointer below.
    let schema = unsafe { handle(schema, "schema") }?;
    let attribute = unsafe { text(attribute, "attribute") }?;
    let records = unsafe { handle(records, "records") }?;
    placed(&schema.0, &records.0)?;
    Ok((&schema.0, attribute, &records.0))
}

/// Refuses, as input, a record that keeps an attribute where its kind does not belong — the core would take it for a
/// broken contract and panic, which leaves what it was changing unusable.
fn placed<'a>(
    schema: &Schema,
    records: impl IntoIterator<Item = &'a Record>,
) -> Result<(), Failure> {
    for record in records {
        schema
            .check_placement(&record.values, &record.sets)
            .map_err(|m| input(format!("record '{}': {m}", record.id)))?;
    }
    Ok(())
}

/// The thresholds a layer selection chose, with the replay's decision records when they were kept.
fn chosen(thresholds: &LayerThresholds, decisions: Option<Vec<Value>>) -> Value {
    let mut value = json!({ "thresholds": thresholds });
    if let Some(decisions) = decisions {
        value["decisions"] = Value::Array(decisions);
    }
    value
}

fn keep(decisions: &mut Option<Vec<Value>>, record: &DecisionRecord) {
    if let Some(kept) = decisions.as_mut() {
        kept.push(serde_json::to_value(record).expect("a decision record serializes"));
    }
}

/// Replays `records` in order and chooses the decided attribute's key and similar record thresholds for `options`
/// (JSON — see the options above; `rule` picks the selection rule). Keeps as the result `{"thresholds": …}` in the
/// core's `serde` form, with `"decisions": [ … ]` (the replay's decision records) when `options` asks for them. An
/// attribute that is not decided, takes several values, or options out of range are contract violations
/// (`GIL_PANIC`).
///
/// # Safety
/// `schema` and `records` are live handles; the texts are valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_select_layers(
    schema: *const GilSchema,
    attribute: GilStr,
    records: *const GilRecords,
    options: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let (schema, attribute, records) = unsafe { selection(schema, attribute, records) }?;
        let options = unsafe { read_options(options) }?;
        let mut decisions = options.decisions.then(Vec::new);
        let thresholds = select_layers(
            &mut KeyMemory::default(),
            &mut record_memory(LexicalMemory::default(), options.threads),
            schema,
            attribute,
            records,
            options.choice(),
            LayerOptions {
                rule: options.rule,
                sink: Some(&mut |r| keep(&mut decisions, r)),
            },
        );
        unsafe { result(&chosen(&thresholds, decisions), result_len) }
    })
}

/// Replays `records` in order and chooses the thresholds of the decided attribute's coarse level — on the lookups the
/// value layers leave at the attribute's thresholds — for `options`. The result is as `gil_select_layers`'; the decision
/// records carry what the coarse level made of each lookup. An attribute with no coarse level is a contract violation
/// (`GIL_PANIC`).
///
/// # Safety
/// As [`gil_select_layers`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_select_coarse(
    schema: *const GilSchema,
    attribute: GilStr,
    records: *const GilRecords,
    options: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let (schema, attribute, records) = unsafe { selection(schema, attribute, records) }?;
        let options = unsafe { read_options(options) }?;
        let mut decisions = options.decisions.then(Vec::new);
        let thresholds = select_coarse_with(
            &mut KeyMemory::default(),
            &mut record_memory(LexicalMemory::default(), options.threads),
            schema,
            attribute,
            records,
            options.choice(),
            &mut |r| keep(&mut decisions, r),
        );
        unsafe { result(&chosen(&thresholds, decisions), result_len) }
    })
}

/// Chooses the attributes that should key the decided attribute — each it rests on alone, then greedy additions while the
/// key layer's gain over the target grows — for `options` (`typed_longest` counts typing). Keeps the choice as the result,
/// in the core's `serde` form.
///
/// # Safety
/// As [`gil_select_layers`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_select_key_fields(
    schema: *const GilSchema,
    attribute: GilStr,
    records: *const GilRecords,
    options: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let (schema, attribute, records) = unsafe { selection(schema, attribute, records) }?;
        let options = unsafe { read_options(options) }?;
        let found = select_key_fields(
            schema,
            attribute,
            records,
            options.choice(),
            options.typed_longest,
        );
        let value = serde_json::to_value(found).expect("a key attribute choice serializes");
        unsafe { result(&value, result_len) }
    })
}

/// Chooses, for each attribute that keys the decided attribute, how readily its keys are believed after only a few
/// settlements. Keeps as the result a JSON object of attribute to concentration.
///
/// # Safety
/// `schema` and `records` are live handles; `attribute` is valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_select_key_concentrations(
    schema: *const GilSchema,
    attribute: GilStr,
    records: *const GilRecords,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let (schema, attribute, records) = unsafe { selection(schema, attribute, records) }?;
        let found =
            select_key_concentrations(&mut KeyMemory::default(), schema, attribute, records);
        unsafe { result(&json!(found), result_len) }
    })
}

/// Keeps as the result the schema as JSON in the core's `serde` form — what `gil_schema_new` reads.
///
/// # Safety
/// `schema` is a live handle; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_schema_json(schema: *const GilSchema, result_len: *mut usize) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller.
        let schema = unsafe { handle(schema, "schema") }?;
        let value = serde_json::to_value(&schema.0).expect("a schema serializes");
        unsafe { result(&value, result_len) }
    })
}

/// # Safety
/// As [`iyulab_ffi_support::text`].
unsafe fn layer_thresholds(json: GilStr) -> Result<LayerThresholds, Failure> {
    // SAFETY: forwarded from the caller.
    let json = unsafe { text(json, "thresholds") }?;
    serde_json::from_str(json).map_err(|e| input(format!("layer thresholds do not read: {e}")))
}

/// Hands out through `out` a new schema: `schema` with what a layers selection chose — `thresholds`, as
/// `gil_select_layers` gives them — put on the decided attribute: the key threshold, the memory threshold and its
/// similarity floor, a layer with none chosen left unset. An attribute the schema does not have or that is not decided,
/// or a result that breaks a schema rule, is refused as input, and the error says which.
///
/// # Safety
/// `schema` is a live handle; the texts are valid for the call; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_schema_with_layers(
    schema: *const GilSchema,
    attribute: GilStr,
    thresholds: GilStr,
    out: *mut *mut GilSchema,
) -> i32 {
    // SAFETY: forwarded from the caller.
    unsafe { schema_with(schema, attribute, thresholds, out, Schema::with_layers) }
}

/// As [`gil_schema_with_layers`], for what a coarse selection (`gil_select_coarse`) chose: the thresholds go on the
/// attribute's coarse level, and an attribute with none is refused as input.
///
/// # Safety
/// As [`gil_schema_with_layers`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_schema_with_coarse(
    schema: *const GilSchema,
    attribute: GilStr,
    thresholds: GilStr,
    out: *mut *mut GilSchema,
) -> i32 {
    // SAFETY: forwarded from the caller.
    unsafe { schema_with(schema, attribute, thresholds, out, Schema::with_coarse) }
}

/// # Safety
/// As [`gil_schema_with_layers`].
unsafe fn schema_with(
    schema: *const GilSchema,
    attribute: GilStr,
    thresholds: GilStr,
    out: *mut *mut GilSchema,
    put: fn(&Schema, &str, &LayerThresholds) -> Result<Schema, String>,
) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let schema = unsafe { handle(schema, "schema") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let thresholds = unsafe { layer_thresholds(thresholds) }?;
        let changed = put(&schema.0, attribute, &thresholds).map_err(input)?;
        unsafe { hand_out(out, GilSchema(changed), "out") }
    })
}

/// # Safety
/// As [`iyulab_ffi_support::text`].
unsafe fn tuning(json: GilStr) -> Result<Tuning, Failure> {
    // SAFETY: forwarded from the caller.
    let json = unsafe { text(json, "tuning") }?;
    serde_json::from_str(json).map_err(|e| input(format!("a tuning does not read: {e}")))
}

/// Keeps as the result the chosen settings of the schema's decided attributes, chosen on `settled` records for
/// `target_precision` — a tuning, as JSON in the core's `serde` form, its promises unrecorded.
///
/// # Safety
/// `schema` is a live handle; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_tuning_capture(
    schema: *const GilSchema,
    settled: usize,
    target_precision: f64,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller.
        let schema = unsafe { handle(schema, "schema") }?;
        let tuned = Tuning::capture(&schema.0, settled, target_precision);
        let value = serde_json::to_value(tuned).expect("a tuning serializes");
        unsafe { result(&value, result_len) }
    })
}

/// Keeps as the result the tuning (JSON) with what `attribute`'s layer selection promised — `thresholds`, as
/// `gil_select_layers` gives them — recorded.
///
/// # Safety
/// The texts are valid for the call; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_tuning_promise(
    tuning_json: GilStr,
    attribute: GilStr,
    thresholds: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let mut tuned = unsafe { tuning(tuning_json) }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let thresholds = unsafe { layer_thresholds(thresholds) }?;
        tuned.promise(attribute, &thresholds);
        let value = serde_json::to_value(tuned).expect("a tuning serializes");
        unsafe { result(&value, result_len) }
    })
}

/// Hands out through `out` a new schema: `schema` with the tuning's settings put back on its attributes. A result that
/// breaks a schema rule — a tuning chosen for another schema may name key attributes this one does not have — is refused
/// as input, and the error names the rule.
///
/// # Safety
/// `tuning_json` is valid for the call; `schema` is a live handle; `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_tuning_apply(
    tuning_json: GilStr,
    schema: *const GilSchema,
    out: *mut *mut GilSchema,
) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let tuned = unsafe { tuning(tuning_json) }?;
        let schema = tuned
            .apply(&unsafe { handle(schema, "schema") }?.0)
            .map_err(input)?;
        unsafe { hand_out(out, GilSchema(schema), "out") }
    })
}

/// Keeps as the result why the tuning no longer stands for `schema` with `settled` records — JSON in the core's `serde`
/// form — or `null` while it does.
///
/// # Safety
/// `tuning_json` is valid for the call; `schema` is a live handle; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_tuning_stale(
    tuning_json: GilStr,
    schema: *const GilSchema,
    settled: usize,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let tuned = unsafe { tuning(tuning_json) }?;
        let schema = unsafe { handle(schema, "schema") }?;
        let value =
            serde_json::to_value(tuned.stale(&schema.0, settled)).expect("a reason serializes");
        unsafe { result(&value, result_len) }
    })
}

/// Keeps as the result the usage counts of the decisions `records` settled, by attribute — a JSON object of attribute to
/// counts in the core's `serde` form.
///
/// # Safety
/// `records` is a live records handle; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_usage(records: *const GilRecords, result_len: *mut usize) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let records = &unsafe { handle(records, "records") }?.0;
        unsafe { result(&json!(usage(records)), result_len) }
    })
}

/// A planner for the decided single-valued attribute `attribute` of `schema` (not off; the planner keeps its own copy of
/// the schema), the observed attributes `askable` — `askable_len` texts — a person could be asked for (at least one,
/// none twice), and the share `budget` (above 0, at most 1) of the typing asking every attribute every time would take
/// that asking may spend, handed out through `out`. A rule broken is refused (`GIL_INVALID_INPUT`), the message naming
/// the first.
///
/// # Safety
/// `schema` is a live handle; `attribute` and the `askable_len` texts at `askable` are valid for the call; `out` points
/// to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_ask_planner_new(
    schema: *const GilSchema,
    attribute: GilStr,
    askable: *const GilStr,
    askable_len: usize,
    budget: f64,
    out: *mut *mut GilAskPlanner,
) -> i32 {
    call(|| {
        if out.is_null() {
            return Err(argument("out is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let schema = unsafe { handle(schema, "schema") }?;
        let attribute = unsafe { text(attribute, "attribute") }?;
        let askable = unsafe { slice(askable, askable_len, "askable") }?
            .iter()
            .map(|a| unsafe { text(*a, "an askable attribute") })
            .collect::<Result<Vec<_>, Failure>>()?;
        let planner = AskPlanner::new(&schema.0, attribute, askable, budget).map_err(input)?;
        let planned = GilAskPlanner {
            planner,
            broken: AtomicBool::new(false),
        };
        unsafe { hand_out(out, planned, "out") }
    })
}

/// Frees a planner; null is ignored.
///
/// # Safety
/// `planner` is null or a live planner handle, not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_ask_planner_free(planner: *mut GilAskPlanner) {
    // SAFETY: forwarded from the caller.
    unsafe { release(planner) }
}

/// Learns from `records`, in order — settled records of the planner's schema, in settlement order — asking `decider`
/// for the decided attribute's answer with and without each askable attribute a record filled. The decider's memories
/// may hold a record being learned, but none settled after it. A record without a value for the decided attribute
/// teaches nothing.
///
/// # Safety
/// `planner`, `decider` and `records` are live handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_ask_planner_learn(
    planner: *mut GilAskPlanner,
    decider: *const GilDecider,
    records: *const GilRecords,
) -> i32 {
    let status = call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let planned = unsafe { live_mut(planner) }?;
        let memories = &unsafe { live(decider) }?.decider;
        let records = &unsafe { handle(records, "records") }?.0;
        placed(planned.planner.schema(), records)?;
        for record in records {
            planned.planner.learn(memories, record);
        }
        Ok(())
    });
    // SAFETY: forwarded from the caller.
    unsafe { break_on_panic(status, planner) };
    status
}

/// Keeps as the result the advice for an open record — a JSON array of `{attribute, gain, ask, threshold}`: every
/// askable attribute the record has no value for, by gain per character, highest first, each marked whether to ask.
/// `answer` is the decided attribute's answer for the record as it stands (JSON, as an answer is given); `values` the
/// record's values as they stand (a JSON object of strings). An answer for another attribute is refused
/// (`GIL_INVALID_INPUT`).
///
/// # Safety
/// `planner` is a live handle; `answer` and `values` are valid for the call; `result_len` points to one writable
/// `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_ask_planner_advise(
    planner: *const GilAskPlanner,
    answer: GilStr,
    values: GilStr,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let planned = unsafe { live(planner) }?;
        let answer: Answer = serde_json::from_str(unsafe { text(answer, "answer") }?)
            .map_err(|e| input(format!("an answer does not read: {e}")))?;
        let values: BTreeMap<String, String> =
            serde_json::from_str(unsafe { text(values, "values") }?)
                .map_err(|e| input(format!("values are an object of strings: {e}")))?;
        if answer.attribute != planned.planner.attribute() {
            return Err(input(format!(
                "the answer is for '{}', not '{}'",
                answer.attribute,
                planned.planner.attribute()
            )));
        }
        let advice = planned.planner.advise(&answer, &values);
        let value = serde_json::to_value(advice).expect("advice serializes");
        unsafe { result(&value, result_len) }
    })
}

/// A lexical memory comparing runs of `min_gram` to `max_gram` Unicode scalars (at least 1, and `max_gram` at least
/// `min_gram` — else `GIL_INVALID_INPUT`), its lookups split among at most `threads` threads (0: the core's default,
/// 1: none), handed out through `out`.
///
/// # Safety
/// `out` points to one writable pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_new(
    min_gram: usize,
    max_gram: usize,
    threads: usize,
    out: *mut *mut GilLexicalMemory,
) -> i32 {
    call(|| {
        if min_gram < 1 {
            return Err(input("min_gram must be at least 1"));
        }
        if max_gram < min_gram {
            return Err(input("max_gram must be at least min_gram"));
        }
        let memory = GilLexicalMemory {
            memory: record_memory(LexicalMemory::new(min_gram, max_gram), threads),
            broken: AtomicBool::new(false),
        };
        // SAFETY: forwarded from the caller.
        unsafe { hand_out(out, memory, "out") }
    })
}

/// Frees a lexical memory; null is ignored.
///
/// # Safety
/// `memory` is null or a live lexical memory handle, not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_free(memory: *mut GilLexicalMemory) {
    // SAFETY: forwarded from the caller.
    unsafe { release(memory) }
}

/// Writes to `count` how many answers the task's memory holds.
///
/// # Safety
/// `memory` is a live handle; `task` is valid for the call; `count` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_count(
    memory: *const GilLexicalMemory,
    task: GilStr,
    count: *mut usize,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let memory = &unsafe { live(memory) }?.memory;
        let task = unsafe { text(task, "task") }?;
        unsafe { write(count, memory.count(task), "count") }
    })
}

/// Remembers `answer` for the text `state` under `key` in the task's memory, replacing what the key held; a key
/// remembered again keeps its place among ties.
///
/// # Safety
/// `memory` is a live handle; the texts are valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_remember(
    memory: *mut GilLexicalMemory,
    task: GilStr,
    key: GilStr,
    state: GilStr,
    answer: GilStr,
) -> i32 {
    let status = call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let task = unsafe { text(task, "task") }?;
        let key = unsafe { text(key, "key") }?;
        let state = unsafe { text(state, "state") }?;
        let answer = unsafe { text(answer, "answer") }?;
        let memory = &mut unsafe { live_mut(memory) }?.memory;
        memory.remember(task, key, state, answer);
        Ok(())
    });
    // SAFETY: forwarded from the caller.
    unsafe { break_on_panic(status, memory) };
    status
}

/// Forgets what `key` held in the task's memory, if anything.
///
/// # Safety
/// `memory` is a live handle; the texts are valid for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_forget(
    memory: *mut GilLexicalMemory,
    task: GilStr,
    key: GilStr,
) -> i32 {
    let status = call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let task = unsafe { text(task, "task") }?;
        let key = unsafe { text(key, "key") }?;
        let memory = &mut unsafe { live_mut(memory) }?.memory;
        memory.forget(task, key);
        Ok(())
    });
    // SAFETY: forwarded from the caller.
    unsafe { break_on_panic(status, memory) };
    status
}

/// Keeps as the result the `count` (at least 1 — else `GIL_INVALID_INPUT`) remembered texts most similar to `state` —
/// a JSON array of `{key, similarity, answer}`, most similar first, on a tie the one remembered first; empty when the
/// task's memory is empty.
///
/// # Safety
/// `memory` is a live handle; the texts are valid for the call; `result_len` points to one writable `usize`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_lexical_memory_nearest(
    memory: *const GilLexicalMemory,
    task: GilStr,
    state: GilStr,
    count: usize,
    result_len: *mut usize,
) -> i32 {
    call(|| {
        if result_len.is_null() {
            return Err(argument("result_len is null"));
        }
        if count < 1 {
            return Err(input("count must be at least 1"));
        }
        // SAFETY: forwarded from the caller, for every pointer below.
        let memory = &unsafe { live(memory) }?.memory;
        let task = unsafe { text(task, "task") }?;
        let state = unsafe { text(state, "state") }?;
        let found: Vec<Value> = memory
            .nearest(task, state, count)
            .into_iter()
            .map(|m| json!({"key": m.source, "similarity": m.similarity, "answer": m.answer}))
            .collect();
        unsafe { result(&Value::Array(found), result_len) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> String {
        let len = unsafe { gil_last_error(std::ptr::null_mut(), 0) };
        let mut buffer = vec![0u8; len];
        unsafe { gil_last_error(buffer.as_mut_ptr(), len) };
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn a_panic_in_a_changing_call_breaks_the_handles_it_was_changing() {
        let mut decider = std::ptr::null_mut();
        assert_eq!(unsafe { gil_decider_new(false, 0, &mut decider) }, GIL_OK);

        // A refusal is not a panic: the handle stays usable.
        unsafe { break_on_panic(GIL_INVALID_INPUT, decider) };
        let status = unsafe { gil_decider_put(decider, std::ptr::null(), std::ptr::null()) };
        assert_eq!(status, GIL_INVALID_ARGUMENT, "{}", last_error());

        // After a panic every call with it is refused, reading ones too, until it is freed.
        unsafe { break_on_panic(GIL_PANIC, decider) };
        let status = unsafe { gil_decider_put(decider, std::ptr::null(), std::ptr::null()) };
        assert_eq!(status, GIL_BROKEN);
        assert!(last_error().starts_with("the decider is broken"));
        let mut len = 0;
        let empty = GilStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        let status = unsafe {
            gil_decider_answer(
                decider,
                std::ptr::null(),
                empty,
                empty,
                empty,
                false,
                &mut len,
            )
        };
        assert_eq!(status, GIL_BROKEN);

        unsafe { break_on_panic(GIL_PANIC, std::ptr::null::<GilDecider>()) }; // null: nothing to mark
        unsafe { gil_decider_free(decider) };
    }
}
