# Changelog — Gil review (`iyulab-gil-review`, .NET `Gil.Review`)

The Rust crate and its .NET binding share this version line; 0.1.0 is the first release of both. Versions follow `0.x`: a minor release may change the public API, and each such change is listed under **Breaking**
with what to do.

## Unreleased

### Added

- **`allocate_neyman_with_minimum`, the Neyman allocation with a minimum number of records for every stratum.** Neyman
  puts the fewest records where the pilot saw the least spread, so a small, noisy stratum could end up with a single
  record and an error of its own far above the rest. Each stratum is first given `min(minimum, its population)` records
  and the rest of the budget is shared by `allocate_neyman` over what each has left. The minimum is capped at
  `⌊budget / 2 / strata⌋`, so the minimums never take more than half the budget; a minimum of 0 or 1, or a budget too
  small for 2 a stratum, allocates exactly as `allocate_neyman`, which is unchanged. In replays on two public
  classification sets with the minimum at 10, the worst stratum's error fell to 0.38 – 0.53 of `allocate_neyman`'s in
  the set with small strata, for 5 – 13% more error overall across budgets of 3 – 15% of the answered records; without
  the cap, a minimum of 10 took 23% more overall at one budget and could not be met at the smallest ones. The .NET
  binding has the overload `ReviewSampler.AllocateNeyman(strata, budget, minimumPerStratum)`.

### Changed

- **The C ABI is version 2** (`gil_review_abi_version`): it adds `gil_review_allocate_neyman_with_minimum`. Nothing else
  changes. The package carries the native library built with its binding, as before.

## 0.1.1

### Added

- **musl-based Linux.** The `Gil.Review` package also carries the native library for `linux-musl-x64` and
  `linux-musl-arm64` (musl 1.2 or later, such as Alpine 3.17 or later), in place of the glibc build the .NET host
  fell back to there — which stopped at the first call on x64. The crate is unchanged.

### Fixed

- **`ReviewSampler.MinimumDesignDeviation` no longer loads the native library.** It is computed in managed code, so a
  host that keeps it in a static field does not fail at type initialization where the library cannot load.

## 0.1.0

### Added

- **.NET binding `Gil.Review`** (`bindings/csharp`): `ReviewSampler`, `ReviewEstimator` and `ReviewAgreement` over a
  flat C ABI (`crates/gil-review-ffi` — strings as UTF-8, results in caller buffers, a status and a message for every
  refusal, no panic across the boundary), AOT compatible. It reproduces the review golden bit for bit; the package
  carries the native library under `runtimes/<rid>/native/`.
- **The sample**: `draw_pilot` (a seeded pilot draw over caller-formed strata — ids in ordinal order, a partial
  Fisher–Yates shuffle, a stratum no larger than the request taken whole), `allocate_neyman` (capped Neyman
  allocation with largest-remainder rounding, planned against `minimum_design_deviation()`), the `SAMPLE_*` caveats.
- **The estimate**: `estimate` (Hájek stratum means, the weighted sample variance or — where every score is the same
  — `unobserved_variation_bound`, the finite-population correction, a normal interval left unclamped, and Rao and
  Wu's rescaled bootstrap with nearest-rank percentiles), `estimate_bounds` (each bound of a band through the same
  estimate), `effective_n`, the `ESTIMATE_*` caveats. With every weight 1 it is the reference arithmetic.
- **`unobserved_variation_bound` in plain arithmetic**: `0.025^(1/n)` by `NEWTON_STEPS` (16) Newton steps from 1,
  then the lower value of the cycle the iteration ends in — in the last bits it can alternate between two
  neighbours. Every n from 1 to 10⁶ enters that cycle within 10 steps, and the root is within two ulps of `powf`.
- **Agreement**: `agreement` over (item, candidate) pairs judged exactly twice — observed agreement, Cohen's κ (NaN when
  both reviewers used one outcome), Gwet's AC1, the agreement specific to each outcome and `prevalence_max`; the
  first reviewer is the one whose id comes first.
- **A language-neutral golden** (`tests/fixtures/review/golden.jsonl`): fixed inputs through the draw, the allocation,
  the bound, the estimate (plain and weighted, with and without a bootstrap), a band and agreement — each double in its
  shortest round-trip form — for a binding to reproduce, and a coverage test: over 100 seeded pilots of a population
  with a known census, the normal interval contains it at least 90 times.
- **The generator**: SplitMix64 into xoshiro256\*\*, integers by Lemire's method — the same sample from the same seed
  on every target.
