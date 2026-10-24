# Gil.Review

Review statistics for checking automated decisions by hand, for .NET — a binding to the Rust crate
`iyulab-gil-review` ([repository](https://github.com/iyulab/gil)):

- **which records to review** — a seeded pilot draw over strata you form (`ReviewSampler.DrawPilot`), then the Neyman
  allocation of the round after it (`ReviewSampler.AllocateNeyman`);
- **what the reviewed scores say about the whole population** — a stratified, weighted estimate with a normal and a
  bootstrap interval (`ReviewEstimator.Estimate`), and a band for items the review could not settle
  (`ReviewEstimator.EstimateBounds`);
- **how far two reviewers agree** — Cohen's κ, Gwet's AC1 and the agreement specific to each outcome
  (`ReviewAgreement.Compute`).

Every draw uses one generator, so the same seed gives the same sample on every platform. Record ids are ordered by their
UTF-8 bytes. Nothing is clamped or silently repaired: an assumption that does not hold is reported in `Caveats`, and an
input that would estimate another population throws `ArgumentException`.

```csharp
using Gil.Review;

var sample = ReviewSampler.DrawPilot(
    [new PopulationStratum("answered", ["d1", "d2", "d3"]), new PopulationStratum("abstained", ["d4", "d5"])],
    perStratum: 30,
    seed: 7);
// Both strata are smaller than the pilot: taken whole.
Console.WriteLine(sample.SelectedCount); // 5
```

The package carries the native library for linux-x64, linux-arm64, linux-musl-x64, linux-musl-arm64, osx-x64,
osx-arm64, win-x64 and win-arm64. A Native AOT application publishes it next to the executable.

Licensed under Apache-2.0.
