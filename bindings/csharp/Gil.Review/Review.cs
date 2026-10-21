namespace Gil.Review;

/// <summary>Which records to review: a seeded pilot draw over strata you form, then the Neyman allocation of the
/// round after it. Every draw uses one generator, so the same seed gives the same sample on every platform.</summary>
public static class ReviewSampler
{
    /// <summary>Records drawn from each stratum in a pilot, before there is a variance to allocate against.</summary>
    public const int PilotRecordsPerStratum = 30;

    /// <summary>The floor on a stratum's deviation that allocation plans against: √(0.05 · 0.95).</summary>
    public static double MinimumDesignDeviation => Native.MinimumDesignDeviation();

    /// <summary>A pilot draw: <paramref name="perStratum"/> records from every stratum. Within a stratum the ids are
    /// put in ordinal order of their UTF-8 bytes and a partial Fisher–Yates shuffle takes the sample; a stratum with no
    /// more records than asked for is taken whole.</summary>
    /// <exception cref="ArgumentException">Fewer than one record per stratum is asked for, a stratum has no name or
    /// shares one, or a record is in two strata.</exception>
    public static unsafe ReviewSample DrawPilot(
        IReadOnlyList<PopulationStratum> strata, int perStratum = PilotRecordsPerStratum, ulong seed = 0)
    {
        ArgumentNullException.ThrowIfNull(strata);
        ArgumentOutOfRangeException.ThrowIfNegative(perStratum);
        var ids = new List<string>();
        var names = new string[strata.Count];
        for (var i = 0; i < strata.Count; i++)
        {
            names[i] = strata[i].Name;
            ids.AddRange(strata[i].RecordIds);
        }

        var nameBlock = Utf8Block.Of(names);
        var idBlock = Utf8Block.Of(ids);
        var native = new Native.PopulationStratum[strata.Count];
        var records = new Native.Str[ids.Count];
        var drawn = new Native.DrawnStratum[strata.Count];
        var selected = new uint[ids.Count];
        uint caveats = 0;
        fixed (byte* namesPinned = nameBlock.Bytes)
        fixed (byte* idsPinned = idBlock.Bytes)
        {
            var first = 0;
            for (var i = 0; i < strata.Count; i++)
            {
                var count = strata[i].RecordIds.Count;
                native[i] = new() { Name = nameBlock.At(namesPinned, i), First = (nuint)first, Count = (nuint)count };
                first += count;
            }

            for (var i = 0; i < ids.Count; i++)
            {
                records[i] = idBlock.At(idsPinned, i);
            }

            fixed (Native.PopulationStratum* s = native)
            fixed (Native.Str* r = records)
            fixed (Native.DrawnStratum* d = drawn)
            fixed (uint* sel = selected)
            {
                Native.Check(Native.DrawPilot(
                    s, (nuint)native.Length, r, (nuint)records.Length, (uint)perStratum, seed,
                    d, sel, (nuint)selected.Length, &caveats));
            }
        }

        var sampled = new List<SampledStratum>();
        var at = 0;
        for (var i = 0; i < strata.Count; i++)
        {
            if (drawn[i].Population == 0)
            {
                continue; // an empty stratum is left out of the sample
            }

            var own = strata[i].RecordIds;
            var chosen = new string[(int)drawn[i].Selected];
            for (var k = 0; k < chosen.Length; k++)
            {
                chosen[k] = own[(int)selected[at++]];
            }

            sampled.Add(new SampledStratum(strata[i].Name, (long)drawn[i].Population, chosen));
        }

        return new ReviewSample(sampled, perStratum, seed, (SampleCaveats)caveats);
    }

    /// <summary>The Neyman allocation of <paramref name="budget"/> records: proportional to each stratum's population
    /// times its deviation (at least <see cref="MinimumDesignDeviation"/>), at least one and at most the population
    /// per stratum, what is cut redistributed until no stratum is capped; the remainder after rounding down goes by
    /// largest fractional part, the lower index first on a tie.</summary>
    /// <exception cref="ArgumentException">No strata, a population of 0, a budget below the number of strata, or a
    /// deviation that is negative or not finite.</exception>
    public static unsafe long[] AllocateNeyman(IReadOnlyList<StratumSpread> strata, long budget)
    {
        ArgumentNullException.ThrowIfNull(strata);
        ArgumentOutOfRangeException.ThrowIfNegative(budget);
        var native = new Native.StratumSpread[strata.Count];
        for (var i = 0; i < strata.Count; i++)
        {
            ArgumentOutOfRangeException.ThrowIfNegative(strata[i].Population);
            native[i] = new() { Population = (ulong)strata[i].Population, Deviation = strata[i].Deviation };
        }

        var counts = new ulong[strata.Count];
        fixed (Native.StratumSpread* s = native)
        fixed (ulong* c = counts)
        {
            Native.Check(Native.AllocateNeyman(s, (nuint)native.Length, (ulong)budget, c));
        }

        return Array.ConvertAll(counts, c => (long)c);
    }
}

/// <summary>What the reviewed scores say about the whole population: a stratified, weighted estimate with a normal and
/// a bootstrap interval, and a band for items the review could not settle. Nothing is clamped or silently repaired —
/// an assumption that does not hold is reported in <see cref="Estimate.Caveats"/>.</summary>
public static class ReviewEstimator
{
    /// <summary>The bootstrap replicates an estimate draws unless told otherwise; 0 turns the bootstrap off.</summary>
    public const int BootstrapDraws = 2000;

    /// <summary>The variance an unvarying stratum of <paramref name="reviewed"/> items is given: p(1 − p) at
    /// p = 1 − 0.025^(1/n), the most the sample still allows.</summary>
    public static double UnobservedVariationBound(long reviewed)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(reviewed);
        return Native.UnobservedVariationBound((ulong)reviewed);
    }

    /// <summary>The stratified estimate of the population's mean score, weighted by each stratum's share of the
    /// population and, within a stratum, by each item's weight; the bootstrap is Rao and Wu's.</summary>
    /// <exception cref="ArgumentException">No strata, a stratum with no name or a shared one, a population of 0, an item
    /// naming a stratum that is not there or with a score outside [0, 1] or a weight not above 0, a stratum with no
    /// item, or a stratum with more items than records.</exception>
    public static unsafe Estimate Estimate(
        IReadOnlyList<Stratum> strata, IReadOnlyList<ScoredItem> items, int replicates = BootstrapDraws,
        ulong seed = 0)
    {
        ArgumentNullException.ThrowIfNull(strata);
        ArgumentNullException.ThrowIfNull(items);
        ArgumentOutOfRangeException.ThrowIfNegative(replicates);
        var names = Utf8Block.Of(strata.Select(s => s.Name).ToArray());
        var native = Items(items);
        Native.Estimate output;
        fixed (byte* pinned = names.Bytes)
        {
            var s = Strata(strata, names, pinned);
            fixed (Native.Stratum* sp = s)
            fixed (Native.ScoredItem* ip = native)
            {
                Native.Check(Native.Estimate_(
                    sp, (nuint)s.Length, ip, (nuint)native.Length, (uint)replicates, seed, &output));
            }
        }

        return From(output);
    }

    /// <summary>The band for items the review could not settle: each unsettled item scored at its worst in
    /// <paramref name="lower"/> and at its best in <paramref name="upper"/>, every other item the same, each set
    /// estimated on its own — so each bound has its own standard error and bootstrap.</summary>
    /// <exception cref="ArgumentException">As <see cref="Estimate"/>, and when the two sets do not give each stratum the
    /// same number of items.</exception>
    public static unsafe (Estimate Low, Estimate High) EstimateBounds(
        IReadOnlyList<Stratum> strata, IReadOnlyList<ScoredItem> lower, IReadOnlyList<ScoredItem> upper,
        int replicates = BootstrapDraws, ulong seed = 0)
    {
        ArgumentNullException.ThrowIfNull(strata);
        ArgumentNullException.ThrowIfNull(lower);
        ArgumentNullException.ThrowIfNull(upper);
        ArgumentOutOfRangeException.ThrowIfNegative(replicates);
        var names = Utf8Block.Of(strata.Select(s => s.Name).ToArray());
        var low = Items(lower);
        var high = Items(upper);
        Native.Estimate lowOut, highOut;
        fixed (byte* pinned = names.Bytes)
        {
            var s = Strata(strata, names, pinned);
            fixed (Native.Stratum* sp = s)
            fixed (Native.ScoredItem* lp = low)
            fixed (Native.ScoredItem* hp = high)
            {
                Native.Check(Native.EstimateBounds(
                    sp, (nuint)s.Length, lp, (nuint)low.Length, hp, (nuint)high.Length, (uint)replicates, seed,
                    &lowOut, &highOut));
            }
        }

        return (From(lowOut), From(highOut));
    }

    private static unsafe Native.Stratum[] Strata(IReadOnlyList<Stratum> strata, Utf8Block names, byte* pinned)
    {
        var native = new Native.Stratum[strata.Count];
        for (var i = 0; i < strata.Count; i++)
        {
            ArgumentOutOfRangeException.ThrowIfNegative(strata[i].Population);
            native[i] = new() { Name = names.At(pinned, i), Population = (ulong)strata[i].Population };
        }

        return native;
    }

    private static Native.ScoredItem[] Items(IReadOnlyList<ScoredItem> items)
    {
        var native = new Native.ScoredItem[items.Count];
        for (var i = 0; i < items.Count; i++)
        {
            var item = items[i];
            // A negative index can name no stratum; the native side reports it as unknown.
            native[i] = new() { Stratum = unchecked((uint)item.Stratum), Score = item.Score, Weight = item.Weight };
        }

        return native;
    }

    private static Estimate From(Native.Estimate e) => new(
        e.Point,
        e.Se,
        new Interval(e.NormalLow, e.NormalHigh),
        new Interval(e.BootstrapLow, e.BootstrapHigh),
        (long)e.SampleSize,
        (long)e.Population,
        e.EffectiveN,
        e.Seed,
        (EstimateCaveats)e.Caveats);
}

/// <summary>How far two reviewers agree: Cohen's κ, Gwet's AC1 and the agreement specific to each outcome.</summary>
public static class ReviewAgreement
{
    /// <summary>Agreement between the two reviewers of every (item, candidate) judged exactly twice; a pair judged once
    /// is passed over. The reviewer whose name comes first in ordinal UTF-8 order is the first rater.</summary>
    /// <exception cref="ArgumentException">Fewer than two categories, an outcome not below the number of categories, a
    /// reviewer judging one pair twice, more than two verdicts on one pair, or no pair judged by two
    /// reviewers.</exception>
    public static unsafe Agreement Compute(IReadOnlyList<Verdict> verdicts, int categories)
    {
        ArgumentNullException.ThrowIfNull(verdicts);
        ArgumentOutOfRangeException.ThrowIfNegative(categories);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(categories, byte.MaxValue);
        var strings = new string[verdicts.Count * 3];
        var outcomes = new byte[verdicts.Count];
        for (var i = 0; i < verdicts.Count; i++)
        {
            var v = verdicts[i];
            if (v.Outcome < 0 || v.Outcome >= categories)
            {
                throw new ArgumentException(
                    $"the verdict on ('{v.Item}', '{v.Candidate}') has outcome {v.Outcome}, not below the {categories} categories",
                    nameof(verdicts));
            }

            (strings[3 * i], strings[(3 * i) + 1], strings[(3 * i) + 2]) = (v.Item, v.Candidate, v.Reviewer);
            outcomes[i] = (byte)v.Outcome;
        }

        var block = Utf8Block.Of(strings);
        var native = new Native.Verdict[verdicts.Count];
        var specific = new double[categories];
        Native.Agreement output;
        fixed (byte* pinned = block.Bytes)
        {
            for (var i = 0; i < native.Length; i++)
            {
                native[i] = new()
                {
                    Item = block.At(pinned, 3 * i),
                    Candidate = block.At(pinned, (3 * i) + 1),
                    Reviewer = block.At(pinned, (3 * i) + 2),
                    Outcome = outcomes[i],
                };
            }

            fixed (Native.Verdict* vp = native)
            fixed (double* sp = specific)
            {
                Native.Check(Native.Agreement_(vp, (nuint)native.Length, (byte)categories, &output, sp));
            }
        }

        return new Agreement(
            (long)output.ComparedPairs, output.Observed, output.Kappa, output.Ac1, specific, output.PrevalenceMax);
    }
}
