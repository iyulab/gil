using AwesomeAssertions;

namespace Gil.Review.Tests;

/// <summary>The allocation with a minimum for every stratum: the counts are those of the Rust crate's tests (and of the
/// reference replay), a minimum of 1 allocates as without one, and the refusals are the same.</summary>
public sealed class MinimumPerStratumTests
{
    private static readonly StratumSpread[] SmallStrata =
    [
        new(60, 0.217945), new(17, 0.0), new(17, 0.235294), new(226, 0.232838), new(452, 0.167134),
    ];

    [Theory]
    [InlineData(23L, new long[] { 3, 3, 3, 5, 9 })]
    [InlineData(39L, new long[] { 5, 4, 4, 10, 16 })]
    [InlineData(77L, new long[] { 10, 8, 8, 20, 31 })]
    [InlineData(116L, new long[] { 14, 11, 11, 31, 49 })]
    public void A_minimum_of_ten_is_capped_to_half_the_budget_and_shares_the_rest(long budget, long[] expected)
    {
        ReviewSampler.AllocateNeyman(SmallStrata, budget, 10).Should().Equal(expected);
    }

    [Fact]
    public void A_minimum_of_one_or_none_allocates_as_without()
    {
        foreach (var budget in new long[] { 5, 23, 77, 400 })
        {
            var plain = ReviewSampler.AllocateNeyman(SmallStrata, budget);
            ReviewSampler.AllocateNeyman(SmallStrata, budget, 0).Should().Equal(plain);
            ReviewSampler.AllocateNeyman(SmallStrata, budget, 1).Should().Equal(plain);
        }
    }

    [Fact]
    public void The_refusals_are_the_same_and_a_negative_minimum_is_refused()
    {
        var refuse = () => ReviewSampler.AllocateNeyman(SmallStrata, 3, 5);
        refuse.Should().Throw<ArgumentException>().WithMessage("*budget*");
        var negative = () => ReviewSampler.AllocateNeyman(SmallStrata, 77, -1);
        negative.Should().Throw<ArgumentException>();
    }
}
