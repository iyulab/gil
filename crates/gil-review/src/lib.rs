//! Review statistics for checking automated decisions by hand: which records to review (a seeded stratified draw and
//! the Neyman allocation after it), what the reviewed scores say about the whole population (a stratified weighted
//! estimate with a normal and a bootstrap interval), and how far two reviewers agree.
//!
//! Every draw uses one generator — SplitMix64 spreading the seed into xoshiro256**, integers by Lemire's method — so the
//! same seed gives the same sample on every target. Nothing here is clamped or silently repaired: an assumption that
//! does not hold is reported in `caveats`, and an input that would estimate another population is an error. No
//! dependencies, no I/O.

pub mod agreement;
mod error;
pub mod estimate;
mod rng;
pub mod sample;

pub use agreement::{Agreement, Verdict, agreement};
pub use error::ReviewError;
pub use estimate::{
    BOOTSTRAP_DRAWS, ESTIMATE_NO_VARIATION_IN_STRATUM, ESTIMATE_STRATUM_IS_CENSUS,
    ESTIMATE_STRATUM_TOO_SMALL, ESTIMATE_WEIGHTS_DEGENERATE, Estimate, Exclusion, Interval,
    MINIMUM_REVIEWED_PER_STRATUM, NEWTON_STEPS, NORMAL_QUANTILE_95, ScoredItem, Stratum, estimate,
    estimate_bounds, unobserved_variation_bound,
};
pub use sample::{
    PILOT_RECORDS_PER_STRATUM, PopulationStratum, ReviewSample, SAMPLE_NO_PARAMETERS,
    SAMPLE_STRATUM_EMPTY, SAMPLE_STRATUM_TAKEN_WHOLE, SampledStratum, StratumSpread,
    allocate_neyman, draw_pilot, minimum_design_deviation,
};

/// The README's example, compiled and run as a test.
#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;
