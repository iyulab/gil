//! A flat C ABI over `iyulab-gil-review`, for the .NET binding: no handles, no callbacks, nothing that outlives a call.
//! Strings go in as (pointer, length) UTF-8; results come back in structures and buffers the caller owns. Every
//! function returns a status — `GIL_REVIEW_OK`, or a code whose message `gil_review_last_error` copies out — and no
//! panic crosses the boundary.

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use gil_review::{
    Estimate, PopulationStratum, ReviewError, ScoredItem, Stratum, StratumSpread, Verdict,
};

/// The call succeeded.
pub const GIL_REVIEW_OK: i32 = 0;
/// The review computation refused its input (`ReviewError`); the message says what was wrong.
pub const GIL_REVIEW_INVALID_INPUT: i32 = 1;
/// The call itself was malformed: a null pointer with a length, text that is not UTF-8, or a buffer too small.
pub const GIL_REVIEW_INVALID_ARGUMENT: i32 = 2;
/// The computation panicked — a defect in the library, reported instead of unwinding into the caller.
pub const GIL_REVIEW_PANIC: i32 = 3;

/// UTF-8 text the caller owns, for the length of one call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewStr {
    pub ptr: *const u8,
    pub len: usize,
}

/// One stratum of a draw: its name and the range `[first, first + count)` of the call's record ids it holds.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewPopulationStratum {
    pub name: GilReviewStr,
    pub first: usize,
    pub count: usize,
}

/// One stratum as drawn: its population and how many of its records were selected. The selected records are the next
/// `selected` entries of the call's index buffer, each an index into the stratum's own records, in the order drawn.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GilReviewDrawnStratum {
    pub population: u64,
    pub selected: u64,
}

/// One stratum's population and deviation, for the Neyman allocation.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewStratumSpread {
    pub population: u64,
    pub deviation: f64,
}

/// One stratum of an estimate: its name and how many records it holds.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewStratum {
    pub name: GilReviewStr,
    pub population: u64,
}

/// One reviewed item: the index of its stratum, its score in [0, 1] and its weight above 0.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewScoredItem {
    pub stratum: u32,
    pub score: f64,
    pub weight: f64,
}

/// A population estimate, as `gil_review::Estimate`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GilReviewEstimate {
    pub point: f64,
    pub se: f64,
    pub normal_low: f64,
    pub normal_high: f64,
    pub bootstrap_low: f64,
    pub bootstrap_high: f64,
    pub sample_size: u64,
    pub population: u64,
    pub effective_n: f64,
    pub seed: u64,
    pub caveats: u32,
}

/// One reviewer's verdict on one (item, candidate).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GilReviewVerdict {
    pub item: GilReviewStr,
    pub candidate: GilReviewStr,
    pub reviewer: GilReviewStr,
    pub outcome: u8,
}

/// Agreement between two reviewers, as `gil_review::Agreement`; the agreement specific to each outcome goes to the
/// call's buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GilReviewAgreement {
    pub compared_pairs: u64,
    pub observed: f64,
    pub kappa: f64,
    pub ac1: f64,
    pub prevalence_max: f64,
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A refusal on its way out: the status and the message `gil_review_last_error` will give.
struct Failure(i32, String);

impl From<ReviewError> for Failure {
    fn from(e: ReviewError) -> Self {
        Self(GIL_REVIEW_INVALID_INPUT, e.to_string())
    }
}

fn argument(message: impl Into<String>) -> Failure {
    Failure(GIL_REVIEW_INVALID_ARGUMENT, message.into())
}

/// Runs one call: clears the last error, keeps a panic from unwinding out, and turns the outcome into a status.
fn call(f: impl FnOnce() -> Result<(), Failure>) -> i32 {
    LAST_ERROR.with(|e| e.borrow_mut().clear());
    let outcome = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "the review computation panicked".to_owned());
        Err(Failure(GIL_REVIEW_PANIC, message))
    });
    match outcome {
        Ok(()) => GIL_REVIEW_OK,
        Err(Failure(status, message)) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = message);
            status
        }
    }
}

/// # Safety
/// `ptr` is null with `len` 0, or points to `len` readable values that stay valid for `'a`.
unsafe fn slice<'a, T>(ptr: *const T, len: usize, what: &str) -> Result<&'a [T], Failure> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(argument(format!("{what} is null but its length is {len}")));
    }
    // SAFETY: non-null, and the caller guarantees `len` readable values for 'a.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// # Safety
/// As [`slice`], writable.
unsafe fn slice_mut<'a, T>(ptr: *mut T, len: usize, what: &str) -> Result<&'a mut [T], Failure> {
    if len == 0 {
        return Ok(&mut []);
    }
    if ptr.is_null() {
        return Err(argument(format!("{what} is null but its length is {len}")));
    }
    // SAFETY: non-null, and the caller guarantees `len` writable values for 'a, aliased by nothing else.
    Ok(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// # Safety
/// As [`slice`], for the text's bytes.
unsafe fn text<'a>(s: GilReviewStr, what: &str) -> Result<&'a str, Failure> {
    // SAFETY: forwarded from the caller.
    let bytes = unsafe { slice(s.ptr, s.len, what) }?;
    std::str::from_utf8(bytes).map_err(|_| argument(format!("{what} is not UTF-8")))
}

/// # Safety
/// `out` is null or points to one writable `T`.
unsafe fn write<T>(out: *mut T, value: T, what: &str) -> Result<(), Failure> {
    if out.is_null() {
        return Err(argument(format!("{what} is null")));
    }
    // SAFETY: non-null, and the caller guarantees one writable T.
    unsafe { out.write(value) };
    Ok(())
}

fn flat(e: &Estimate) -> GilReviewEstimate {
    GilReviewEstimate {
        point: e.point,
        se: e.se,
        normal_low: e.normal.low,
        normal_high: e.normal.high,
        bootstrap_low: e.bootstrap.low,
        bootstrap_high: e.bootstrap.high,
        sample_size: e.sample_size,
        population: e.population,
        effective_n: e.effective_n,
        seed: e.seed,
        caveats: e.caveats,
    }
}

/// Copies the message of the last refused call on this thread into `buffer` as UTF-8 (no terminator) and returns its
/// full length in bytes — call with a capacity of 0 to learn the length. Nothing is written when the message does not
/// fit. An empty message means the last call succeeded.
///
/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_review_last_error(buffer: *mut u8, capacity: usize) -> usize {
    LAST_ERROR.with(|e| {
        let message = e.borrow();
        let bytes = message.as_bytes();
        if !buffer.is_null() && capacity >= bytes.len() {
            // SAFETY: the caller guarantees `capacity` writable bytes, and the message fits.
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
        }
        bytes.len()
    })
}

/// `gil_review::draw_pilot` over strata whose records are ranges of `record_ids`. Writes one `GilReviewDrawnStratum`
/// per stratum given (an empty stratum gets 0 and 0) to `drawn`, the selected records' indices within their strata to
/// `selected` (stratum by stratum, in the order drawn — at most as many as `record_ids`), and the sample's caveats to
/// `caveats`.
///
/// # Safety
/// Every pointer is null with its length 0 or points to that many valid values; `drawn` holds `strata_len` and
/// `selected` holds `selected_capacity` writable values; `caveats` points to one writable `u32`.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn gil_review_draw_pilot(
    strata: *const GilReviewPopulationStratum,
    strata_len: usize,
    record_ids: *const GilReviewStr,
    record_ids_len: usize,
    per_stratum: u32,
    seed: u64,
    drawn: *mut GilReviewDrawnStratum,
    selected: *mut u32,
    selected_capacity: usize,
    caveats: *mut u32,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller, for every pointer below.
        let strata = unsafe { slice(strata, strata_len, "strata") }?;
        let ids = unsafe { slice(record_ids, record_ids_len, "record_ids") }?;
        let drawn = unsafe { slice_mut(drawn, strata_len, "drawn") }?;
        let selected = unsafe { slice_mut(selected, selected_capacity, "selected") }?;
        if caveats.is_null() {
            return Err(argument("caveats is null"));
        }

        let ids: Vec<&str> = ids
            .iter()
            .map(|&s| unsafe { text(s, "a record id") })
            .collect::<Result<_, _>>()?;
        let mut names = Vec::with_capacity(strata.len());
        let mut members = Vec::with_capacity(strata.len());
        for s in strata {
            let end = s.first.checked_add(s.count).filter(|&e| e <= ids.len());
            let Some(end) = end else {
                return Err(argument("a stratum's record range runs past record_ids"));
            };
            names.push(unsafe { text(s.name, "a stratum name") }?);
            members.push(&ids[s.first..end]);
        }
        let population: Vec<PopulationStratum> = names
            .iter()
            .zip(&members)
            .map(|(&name, &record_ids)| PopulationStratum { name, record_ids })
            .collect();
        let sample = gil_review::draw_pilot(&population, per_stratum, seed)?;

        let total: usize = sample.strata.iter().map(|s| s.selected.len()).sum();
        if total > selected.len() {
            return Err(argument(format!(
                "selected holds {} indices but the draw selected {total}",
                selected.len()
            )));
        }
        let mut next = sample.strata.iter().peekable();
        let mut at = 0;
        for ((slot, &name), &records) in drawn.iter_mut().zip(&names).zip(&members) {
            *slot = GilReviewDrawnStratum::default();
            // Empty strata are left out of the sample; the rest keep the order given.
            let Some(s) = next.next_if(|s| s.name == name) else {
                continue;
            };
            let index: HashMap<&str, usize> =
                records.iter().enumerate().map(|(i, &id)| (id, i)).collect();
            for id in &s.selected {
                selected[at] = u32::try_from(index[id.as_str()])
                    .map_err(|_| argument("a stratum holds more than u32::MAX records"))?;
                at += 1;
            }
            *slot = GilReviewDrawnStratum {
                population: s.population,
                selected: s.selected.len() as u64,
            };
        }
        // SAFETY: checked non-null above; the caller guarantees one writable u32.
        unsafe { write(caveats, sample.caveats, "caveats") }
    })
}

/// `gil_review::allocate_neyman`: writes one count per stratum to `counts`.
///
/// # Safety
/// `strata` points to `strata_len` valid values and `counts` to `strata_len` writable ones (or both null with 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_review_allocate_neyman(
    strata: *const GilReviewStratumSpread,
    strata_len: usize,
    budget: u64,
    counts: *mut u64,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller.
        let strata = unsafe { slice(strata, strata_len, "strata") }?;
        let counts = unsafe { slice_mut(counts, strata_len, "counts") }?;
        let spreads: Vec<StratumSpread> = strata
            .iter()
            .map(|s| StratumSpread {
                population: s.population,
                deviation: s.deviation,
            })
            .collect();
        counts.copy_from_slice(&gil_review::allocate_neyman(&spreads, budget)?);
        Ok(())
    })
}

/// `gil_review::minimum_design_deviation`.
#[unsafe(no_mangle)]
pub extern "C" fn gil_review_minimum_design_deviation() -> f64 {
    gil_review::minimum_design_deviation()
}

/// `gil_review::unobserved_variation_bound`.
#[unsafe(no_mangle)]
pub extern "C" fn gil_review_unobserved_variation_bound(reviewed: u64) -> f64 {
    gil_review::unobserved_variation_bound(reviewed)
}

/// # Safety
/// As the callers'.
unsafe fn estimate_inputs<'a>(
    strata: *const GilReviewStratum,
    strata_len: usize,
) -> Result<Vec<Stratum<'a>>, Failure> {
    // SAFETY: forwarded from the caller.
    let strata = unsafe { slice(strata, strata_len, "strata") }?;
    strata
        .iter()
        .map(|s| {
            Ok(Stratum {
                name: unsafe { text(s.name, "a stratum name") }?,
                population: s.population,
            })
        })
        .collect()
}

/// # Safety
/// As the callers'.
unsafe fn items(ptr: *const GilReviewScoredItem, len: usize) -> Result<Vec<ScoredItem>, Failure> {
    // SAFETY: forwarded from the caller.
    Ok(unsafe { slice(ptr, len, "items") }?
        .iter()
        .map(|i| ScoredItem {
            stratum: i.stratum,
            score: i.score,
            weight: i.weight,
        })
        .collect())
}

/// `gil_review::estimate`: writes the estimate to `out`.
///
/// # Safety
/// `strata` and `items` point to that many valid values (or are null with 0); `out` points to one writable estimate.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_review_estimate(
    strata: *const GilReviewStratum,
    strata_len: usize,
    items: *const GilReviewScoredItem,
    items_len: usize,
    replicates: u32,
    seed: u64,
    out: *mut GilReviewEstimate,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller.
        let strata = unsafe { estimate_inputs(strata, strata_len) }?;
        let items = unsafe { self::items(items, items_len) }?;
        let e = gil_review::estimate(&strata, &items, replicates, seed)?;
        unsafe { write(out, flat(&e), "out") }
    })
}

/// `gil_review::estimate_bounds`: writes the two estimates to `low` and `high`.
///
/// # Safety
/// As [`gil_review_estimate`], with both item sets and both outputs.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn gil_review_estimate_bounds(
    strata: *const GilReviewStratum,
    strata_len: usize,
    lower: *const GilReviewScoredItem,
    lower_len: usize,
    upper: *const GilReviewScoredItem,
    upper_len: usize,
    replicates: u32,
    seed: u64,
    low: *mut GilReviewEstimate,
    high: *mut GilReviewEstimate,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller.
        let strata = unsafe { estimate_inputs(strata, strata_len) }?;
        let lower = unsafe { items(lower, lower_len) }?;
        let upper = unsafe { items(upper, upper_len) }?;
        if low.is_null() || high.is_null() {
            return Err(argument("low and high must both be given"));
        }
        let (l, h) = gil_review::estimate_bounds(&strata, &lower, &upper, replicates, seed)?;
        unsafe { write(low, flat(&l), "low") }?;
        unsafe { write(high, flat(&h), "high") }
    })
}

/// `gil_review::agreement`: writes the summary to `out` and the agreement specific to each of the `categories`
/// outcomes to `specific` (NaN for an outcome neither reviewer gave).
///
/// # Safety
/// `verdicts` points to `verdicts_len` valid values (or is null with 0); `out` points to one writable agreement and
/// `specific` to `categories` writable doubles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gil_review_agreement(
    verdicts: *const GilReviewVerdict,
    verdicts_len: usize,
    categories: u8,
    out: *mut GilReviewAgreement,
    specific: *mut f64,
) -> i32 {
    call(|| {
        // SAFETY: forwarded from the caller.
        let verdicts = unsafe { slice(verdicts, verdicts_len, "verdicts") }?;
        let specific = unsafe { slice_mut(specific, usize::from(categories), "specific") }?;
        if out.is_null() {
            return Err(argument("out is null"));
        }
        let verdicts: Vec<Verdict> = verdicts
            .iter()
            .map(|v| {
                Ok(Verdict {
                    item: unsafe { text(v.item, "a verdict's item") }?,
                    candidate: unsafe { text(v.candidate, "a verdict's candidate") }?,
                    outcome: v.outcome,
                    reviewer: unsafe { text(v.reviewer, "a verdict's reviewer") }?,
                })
            })
            .collect::<Result<_, Failure>>()?;
        let a = gil_review::agreement(&verdicts, categories)?;
        specific.copy_from_slice(&a.specific);
        unsafe {
            write(
                out,
                GilReviewAgreement {
                    compared_pairs: a.compared_pairs,
                    observed: a.observed,
                    kappa: a.kappa,
                    ac1: a.ac1,
                    prevalence_max: a.prevalence_max,
                },
                "out",
            )
        }
    })
}
