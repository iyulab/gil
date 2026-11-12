# Gil review (`iyulab-gil-review`)

Review statistics for checking automated decisions by hand, with no dependencies and no I/O:

- **which records to review** — a seeded pilot draw over strata you form (`draw_pilot`), then the Neyman allocation of
  the round after it (`allocate_neyman`), planned against a floor on each stratum's deviation, or with a minimum for
  every stratum (`allocate_neyman_with_minimum`) when a small stratum would otherwise get one or two records;
- **what the reviewed scores say about the whole population** — a stratified, weighted estimate with a normal and a
  bootstrap interval, and a band for items the record could not settle;
- **how far two reviewers agree** — Cohen's κ, Gwet's AC1 and the agreement specific to each outcome.

Every draw uses one generator — SplitMix64 spreading the seed into xoshiro256\*\*, integers by Lemire's method — so the
same seed gives the same sample on every target. Nothing is clamped or silently repaired: an assumption that does not
hold is reported in `caveats`, and an input that would estimate another population is an error.

**Scores of 0 or 1 from a system that is right most of the time** — a stratum whose reviewed items are all right is
then the usual case, not an exception, and the estimate reports `ESTIMATE_NO_VARIATION_IN_STRATUM`. With four strata of
30 reviewed items each, it does so for about 85% of estimates at a precision of 0.97 in every stratum, and for almost
all of them at 0.99 (one stratum of n items is constant with probability pⁿ + (1 − p)ⁿ). Such a stratum's variance
takes the bound for the variation the sample has not shown, and its bootstrap collapses, so read the normal interval.
`is_reliable()` is then false, so for 0/1 scores do not use it as a go/no-go — look at the other caveats one by one.

Add it with `cargo add iyulab-gil-review`; the library is imported as `gil_review`. The example below is a whole
program — paste it into `src/main.rs` and `cargo run`.

```rust
use gil_review::{PopulationStratum, draw_pilot, PILOT_RECORDS_PER_STRATUM};

fn main() -> Result<(), gil_review::ReviewError> {
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
    Ok(())
}
```

From .NET, the `Gil.Review` package binds this crate with the same results (`bindings/csharp` in the repository).

Licensed under Apache-2.0.
