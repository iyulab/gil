# Changelog — Gil review (`iyulab-gil-review`)

Versions follow `0.x`: a minor release may change the public API, and each such change is listed under **Breaking**
with what to do.

## Unreleased

### Added

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
