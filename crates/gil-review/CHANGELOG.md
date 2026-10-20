# Changelog — Gil review (`iyulab-gil-review`)

Versions follow `0.x`: a minor release may change the public API, and each such change is listed under **Breaking**
with what to do.

## Unreleased

### Added

- **The sample**: `draw_pilot` (a seeded pilot draw over caller-formed strata — ids in ordinal order, a partial
  Fisher–Yates shuffle, a stratum no larger than the request taken whole), `allocate_neyman` (capped Neyman
  allocation with largest-remainder rounding, planned against `minimum_design_deviation()`), the `SAMPLE_*` caveats.
- **The generator**: SplitMix64 into xoshiro256\*\*, integers by Lemire's method — the same sample from the same seed
  on every target.
