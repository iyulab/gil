# Gil review (`iyulab-gil-review`)

Review statistics for checking automated decisions by hand, with no dependencies and no I/O:

- **which records to review** — a seeded pilot draw over strata you form (`draw_pilot`), then the Neyman allocation of
  the round after it (`allocate_neyman`), planned against a floor on each stratum's deviation;
- **what the reviewed scores say about the whole population** — a stratified, weighted estimate with a normal and a
  bootstrap interval, and a band for items the record could not settle;
- **how far two reviewers agree** — Cohen's κ, Gwet's AC1 and the agreement specific to each outcome.

Every draw uses one generator — SplitMix64 spreading the seed into xoshiro256\*\*, integers by Lemire's method — so the
same seed gives the same sample on every target. Nothing is clamped or silently repaired: an assumption that does not
hold is reported in `caveats`, and an input that would estimate another population is an error.

```rust
use gil_review::{PopulationStratum, draw_pilot, PILOT_RECORDS_PER_STRATUM};

let answered = ["d1", "d2", "d3"];
let abstained = ["d4", "d5"];
let sample = draw_pilot(
    &[
        PopulationStratum { name: "answered", record_ids: &answered },
        PopulationStratum { name: "abstained", record_ids: &abstained },
    ],
    PILOT_RECORDS_PER_STRATUM,
    7,
)?;
assert_eq!(sample.selected_count(), 5); // both strata are smaller than the pilot: taken whole
# Ok::<(), gil_review::ReviewError>(())
```

From .NET, the `Gil.Review` package binds this crate with the same results (`bindings/csharp` in the repository).

Licensed under Apache-2.0.
