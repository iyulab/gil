using AwesomeAssertions;

namespace Gil.Review.Tests;

/// <summary>An input the review statistics refuse comes back as an <see cref="ArgumentException"/> carrying the
/// native message; strings cross as UTF-8, including text outside the Basic Multilingual Plane.</summary>
public sealed class RefusalTests
{
    [Fact]
    public void A_budget_below_the_number_of_strata_is_refused_with_its_reason()
    {
        StratumSpread[] strata = [new(10, 0.2), new(10, 0.2), new(10, 0.2)];
        var refuse = () => ReviewSampler.AllocateNeyman(strata, 2);
        refuse.Should().Throw<ArgumentException>().WithMessage("*budget*");

        // The binding keeps working after a refusal.
        ReviewSampler.AllocateNeyman(strata, 9).Sum().Should().Be(9);
    }

    [Fact]
    public void A_record_in_two_strata_is_refused_by_name()
    {
        PopulationStratum[] strata = [new("a", ["r1", "r2"]), new("b", ["r2"])];
        var refuse = () => ReviewSampler.DrawPilot(strata, 1, 0);
        refuse.Should().Throw<ArgumentException>().WithMessage("*'r2'*");
    }

    [Fact]
    public void A_stratum_with_no_scored_item_is_refused()
    {
        Stratum[] strata = [new("seen", 10), new("unseen", 10)];
        var refuse = () => ReviewEstimator.Estimate(strata, [new ScoredItem(0, 1.0), new ScoredItem(0, 0.0)]);
        refuse.Should().Throw<ArgumentException>().WithMessage("*'unseen'*");
    }

    [Fact]
    public void An_outcome_outside_the_categories_is_refused()
    {
        var refuse = () => ReviewAgreement.Compute([new Verdict("i", "c", 3, "amy"), new Verdict("i", "c", 0, "zed")], 3);
        refuse.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void Record_ids_outside_the_basic_plane_cross_intact_and_sort_by_their_utf8_bytes()
    {
        // U+1F600 (UTF-8 F0 9F 98 80) sorts after U+FF61 (EF BD A1) by bytes, before it by UTF-16 units — the draw
        // takes the byte order, so taking the stratum whole lists the ids in that order.
        PopulationStratum[] strata = [new("s", ["\uFF61", "\U0001F600", "a"])];
        var sample = ReviewSampler.DrawPilot(strata, 3, 0);
        sample.Strata.Should().ContainSingle().Which.Selected.Should().Equal("a", "\uFF61", "\U0001F600");
        sample.Caveats.Should().Be(SampleCaveats.StratumTakenWhole);
    }

    [Fact]
    public void The_design_floor_is_the_square_root_of_its_formula_and_the_one_the_library_plans_against()
    {
        ReviewSampler.MinimumDesignDeviation.Should().Be(Math.Sqrt(0.05 * 0.95));
        Native.EnsureCompatible();
        ReviewSampler.MinimumDesignDeviation.Should().Be(Native.MinimumDesignDeviation());
    }
}
