using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>A threshold for a judge outside Gil (experimental), chosen from its scores and what people settled.</summary>
public sealed class ScoreThresholdTests
{
    /// <summary>A judge whose score is the chance it is right.</summary>
    private static List<ScoredDecision> Calibrated(int count, int seed)
    {
        var random = new Random(seed);
        var epoch = DateTimeOffset.UnixEpoch;
        return Enumerable.Range(0, count).Select(i =>
        {
            var score = random.NextDouble();
            return new ScoredDecision($"c{i:00000}", epoch.AddSeconds(i), score, random.NextDouble() < score);
        }).ToList();
    }

    [Fact]
    public void ScoresAndOutcomesGiveAThresholdAndAPromise()
    {
        var replay = Replay.SelectScoreThreshold(Calibrated(4_000, 1), new SelectionOptions(0.8, 30));

        var chosen = replay.Chosen!;
        chosen.Threshold.Should().BeInRange(0.7, 0.9);
        chosen.Precision.Should().BeGreaterThanOrEqualTo(0.8);
        chosen.Promise.Should().BeLessThanOrEqualTo(chosen.Precision);
        chosen.Lookups.Should().Be(4_000);
    }

    [Fact]
    public void ADecisionWithNoScoreIsAskedAndNotAnswered()
    {
        var decisions = Calibrated(3_000, 2);
        var plain = Replay.SelectScoreThreshold(decisions, new SelectionOptions(0.8, 30)).Chosen!;
        var silent = Enumerable.Range(0, 1_000)
            .Select(i => new ScoredDecision($"s{i}", DateTimeOffset.UnixEpoch.AddDays(1).AddSeconds(i), null));

        var with = Replay.SelectScoreThreshold([.. decisions, .. silent], new SelectionOptions(0.8, 30)).Chosen!;

        with.Threshold.Should().Be(plain.Threshold);
        with.Lookups.Should().Be(4_000);
        with.AnswerRate.Should().BeLessThan(plain.AnswerRate);
    }

    [Fact]
    public void AScoreThatSaysNothingGetsNoThreshold()
    {
        var random = new Random(3);
        var flat = Enumerable.Range(0, 4_000)
            .Select(i => new ScoredDecision($"c{i}", DateTimeOffset.UnixEpoch.AddSeconds(i), random.NextDouble(), random.NextDouble() < 0.5))
            .ToList();

        Replay.SelectScoreThreshold(flat, new SelectionOptions(0.9, 30)).Chosen.Should().BeNull();
    }

    [Fact]
    public void WhatTheCoreRefusesIsAnArgumentException()
    {
        var one = new List<ScoredDecision> { new("a", DateTimeOffset.UnixEpoch, 0.5) };

        var badTarget = () => Replay.SelectScoreThreshold(one, new SelectionOptions(1.5, 1));
        var badWeight = () => Replay.SelectScoreThreshold([new("a", DateTimeOffset.UnixEpoch, 0.5, true, 0.0)], new SelectionOptions(0.8, 1));

        badTarget.Should().Throw<ArgumentException>();
        badWeight.Should().Throw<ArgumentException>();
    }
}
