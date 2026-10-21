namespace Gil.Review;

/// <summary>One stratum of the population a review is drawn from: its name and every record in it.</summary>
public sealed record PopulationStratum(string Name, IReadOnlyList<string> RecordIds);

/// <summary>One stratum as drawn: its whole membership (the population an estimate is weighted by) and the records
/// selected, in the order drawn.</summary>
public sealed record SampledStratum(string Name, long Population, IReadOnlyList<string> Selected);

/// <summary>What a draw could not do as asked.</summary>
[Flags]
public enum SampleCaveats
{
    None = 0,

    /// <summary>A stratum held no records; it is left out of the sample.</summary>
    StratumEmpty = 1 << 0,

    /// <summary>A stratum held no more records than were asked for and was taken whole.</summary>
    StratumTakenWhole = 1 << 1,

    /// <summary>Set by a caller whose strata were formed without the parameters they need.</summary>
    NoParameters = 1 << 2,
}

/// <summary>A stratified draw for review, reproducible from its seed.</summary>
/// <param name="Strata">The strata drawn, in the order given; an empty stratum is left out.</param>
/// <param name="RequestedPerStratum">The records asked of each stratum.</param>
/// <param name="Seed">The seed the draw was made with.</param>
/// <param name="Caveats">What the draw could not do as asked.</param>
public sealed record ReviewSample(
    IReadOnlyList<SampledStratum> Strata, int RequestedPerStratum, ulong Seed, SampleCaveats Caveats)
{
    /// <summary>How many records a reviewer has to look at.</summary>
    public int SelectedCount => Strata.Sum(s => s.Selected.Count);

    /// <summary>Whether every stratum held records and each gave up as many as were asked for.</summary>
    public bool IsReliable => Caveats == SampleCaveats.None;
}

/// <summary>One stratum's population and the deviation of its scores, for the Neyman allocation.</summary>
public readonly record struct StratumSpread(long Population, double Deviation);

/// <summary>A stratum of the population an estimate is weighted by, fixed before scoring: its name (for messages and
/// reports only) and how many records it holds.</summary>
public readonly record struct Stratum(string Name, long Population);

/// <summary>One reviewed item: the index of its stratum, its score in [0, 1] and its weight — 1 for a plain sample,
/// the inverse of the probability it was drawn with for a weighted one.</summary>
public readonly record struct ScoredItem(int Stratum, double Score, double Weight = 1.0);

/// <summary>An interval, never clamped to [0, 1].</summary>
public readonly record struct Interval(double Low, double High);

/// <summary>Which assumptions behind an estimate's intervals did not hold.</summary>
[Flags]
public enum EstimateCaveats
{
    None = 0,

    /// <summary>A stratum has fewer than two scored items: no sample variance can be read from it, and the bootstrap
    /// keeps its mean.</summary>
    StratumTooSmall = 1 << 0,

    /// <summary>A stratum was reviewed in full: it contributes no sampling variance.</summary>
    StratumIsCensus = 1 << 1,

    /// <summary>A stratum's scores are all the same: its variance is the most the sample still allows, and the
    /// bootstrap collapses there — read the normal interval.</summary>
    NoVariationInStratum = 1 << 2,

    /// <summary>In some stratum the weights leave fewer than half the items' worth of information.</summary>
    WeightsDegenerate = 1 << 3,
}

/// <summary>A population estimate from a stratified, weighted review.</summary>
public sealed record Estimate(
    double Point,
    double StandardError,
    Interval Normal,
    Interval Bootstrap,
    long SampleSize,
    long Population,
    double EffectiveSampleSize,
    ulong Seed,
    EstimateCaveats Caveats)
{
    /// <summary>Whether every assumption the intervals rest on held.</summary>
    public bool IsReliable => Caveats == EstimateCaveats.None;
}

/// <summary>One reviewer's verdict on one (item, candidate): an outcome below the number of categories.</summary>
public readonly record struct Verdict(string Item, string Candidate, int Outcome, string Reviewer);

/// <summary>Agreement between two reviewers over the pairs both judged.</summary>
/// <param name="ComparedPairs">The (item, candidate) pairs both reviewers judged.</param>
/// <param name="Observed">The share of compared pairs on which the two gave the same outcome.</param>
/// <param name="Kappa">Cohen's κ; NaN when both reviewers used one outcome only.</param>
/// <param name="Ac1">Gwet's AC1 — always defined.</param>
/// <param name="Specific">Per outcome: of the times either reviewer gave it, how often both did; NaN for an outcome
/// neither gave.</param>
/// <param name="PrevalenceMax">The largest share of all verdicts compared that one outcome took — reported, never used
/// to judge.</param>
public sealed record Agreement(
    long ComparedPairs, double Observed, double Kappa, double Ac1, IReadOnlyList<double> Specific,
    double PrevalenceMax);
