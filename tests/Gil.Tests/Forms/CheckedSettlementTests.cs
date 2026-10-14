using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>
/// A check settles a filled value again, drawn with a known probability; memory learns it and choosing thresholds counts
/// it as the filled decisions it stands for.
/// </summary>
public sealed class CheckedSettlementTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly FormDefinition Ticket = new(
        "ticket",
        [
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.3 },
        ],
        PromptLanguage.English);

    private static readonly DateTimeOffset Monday = new(2026, 9, 28, 9, 0, 0, TimeSpan.Zero);

    private static SettledDocument Doc(string id, string team, int day, SettlementKind kind, double? probability = null) =>
        new(id, new Dictionary<string, string> { ["summary"] = "vpn drops", ["team"] = team }, Monday.AddDays(day))
        {
            Settlements = new Dictionary<string, SettlementKind> { ["team"] = kind },
            Probabilities = probability is { } p ? new Dictionary<string, double> { ["team"] = p } : new Dictionary<string, double>(),
        };

    [Theory]
    [InlineData(0.0)]
    [InlineData(-0.5)]
    [InlineData(1.5)]
    [InlineData(double.NaN)]
    public void A_check_is_drawn_with_a_probability_above_0_and_at_most_1(double probability) =>
        FluentActions.Invoking(() => Settlement.Check("network", probability)).Should().Throw<ArgumentOutOfRangeException>();

    [Fact]
    public void Only_a_check_carries_a_probability_other_than_1()
    {
        Settlement.Check("network", 0.25).Probability.Should().Be(0.25);
        Settlement.Check("network", 1).Kind.Should().Be(SettlementKind.Check);
        Settlement.Accept("network").Probability.Should().Be(1);
        Settlement.Fill("network").Probability.Should().Be(1);
    }

    [Fact]
    public async Task A_checked_value_is_learned_and_its_probability_travels_with_the_document()
    {
        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        var session = resolver.Open(Ticket, "d1");
        await session.ObserveAsync("summary", "vpn drops", Ct);
        await session.SettleAsync("team", Settlement.Fill("facilities"), Ct);
        await session.SettleAsync("team", Settlement.Check("network", 0.25), Ct);

        var saved = session.Snapshot();
        saved.Settlements["team"].Should().Be(SettlementKind.Check);
        saved.Probabilities["team"].Should().Be(0.25);
        resolver.Open(Ticket, saved).Snapshot().Probabilities["team"].Should().Be(0.25); // reopened as saved

        var candidates = (await resolver.Open(Ticket, "d2").ObserveAsync("summary", "vpn drops", Ct))
            .Single(s => s.Field == "team").Candidates.Select(c => c.Value).ToList();
        candidates.Should().Contain("network").And.NotContain("facilities"); // the check replaced the filled value

        await session.SettleAsync("team", Settlement.Correct("facilities"), Ct);
        session.Snapshot().Probabilities.Should().BeEmpty(); // settled again some other way: no longer a check
    }

    [Fact]
    public async Task Choosing_thresholds_counts_a_check_as_the_decisions_it_stands_for()
    {
        // d2 was one of four filled decisions, drawn with a quarter's chance and found right; d3, settled by a person, was
        // answered wrongly. Counted once each that is one right in two; d2 standing for its four, four right in five.
        SettledDocument[] documents =
        [
            Doc("d1", "network", 0, SettlementKind.Accept),
            Doc("d2", "network", 1, SettlementKind.Check, 0.25),
            Doc("d3", "facilities", 2, SettlementKind.Accept),
        ];

        var replay = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.75, 1);
        replay.Lookups.Should().Be(2);
        var chosen = replay.Chosen!;
        chosen.Precision.Should().BeApproximately(0.8, 1e-12);
        chosen.AnswerRate.Should().BeApproximately(1.0, 1e-12);
        (chosen.Answered, chosen.Lookups).Should().Be((2, 2));
        chosen.AnsweredWeight.Should().BeApproximately(5, 1e-12);
        chosen.EffectiveAnswered.Should().BeApproximately(25.0 / 17, 1e-12); // (4 + 1)² ÷ (4² + 1²)

        var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket, "team", documents, 0.75, 1, Ct);
        layers.Key.Chosen!.Precision.Should().BeApproximately(0.8, 1e-12);

        // The same values entered by a person count once each: one right in two misses the target.
        SettledDocument[] entered = [documents[0], Doc("d2", "network", 1, SettlementKind.Enter), documents[2]];
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", entered, 0.75, 1).Chosen.Should().BeNull();
    }

    [Fact]
    public void A_check_drawn_rarely_weighs_much_but_counts_as_one_answer_toward_the_minimum()
    {
        SettledDocument[] documents =
        [
            Doc("d1", "network", 0, SettlementKind.Accept),
            Doc("d2", "network", 1, SettlementKind.Check, 0.1),
        ];

        var one = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 1).Chosen!;
        one.AnsweredWeight.Should().BeApproximately(10, 1e-12);
        one.EffectiveAnswered.Should().BeApproximately(1, 1e-12);

        var two = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 2);
        two.Chosen.Should().BeNull();
        two.MostPrecise.Should().BeNull(); // ten decisions' weight, one decision's evidence
    }

    [Fact]
    public void Counting_by_kind_takes_a_check_only_when_it_is_named()
    {
        SettledDocument[] documents =
        [
            Doc("d1", "network", 0, SettlementKind.Accept),
            Doc("d2", "network", 1, SettlementKind.Check, 0.25),
        ];

        SettlementKind[] byHand = [SettlementKind.Accept, SettlementKind.Correct, SettlementKind.Enter];
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 1, counting: byHand).Lookups.Should().Be(0);
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 1, counting: [.. byHand, SettlementKind.Check]).Lookups.Should().Be(1);
    }
}
