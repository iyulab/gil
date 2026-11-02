using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Similar candidates (experimental): the similar record layer offering, after the value it voted for, the
/// values that drew less of the vote — never trusted, each scored by its share of the vote.</summary>
public sealed class SimilarCandidatesTests
{
    private static Schema Intake(int candidates) => new("intake", [
        new AttributeDefinition("note", AttributeRole.Observed),
        new AttributeDefinition("category", AttributeRole.Decided) { MemoryThreshold = 0.0, SimilarCandidates = candidates },
    ]);

    private static readonly (string Note, string Category)[] History =
    [
        ("router keeps dropping the connection every evening", "billing"),
        ("router keeps dropping the connection at night", "outage"),
        ("router keeps dropping connection in the evening", "outage"),
        ("the router is dropping the connection every night", "outage"),
        ("invoice shows a charge twice this month", "billing"),
        ("router connection dropped and the invoice was refunded", "refund"),
    ];

    private static SettledRecords Records() => SettledRecords.FromJsonLines(string.Join('\n',
        History.Select((h, i) => JsonSerializer.Serialize(new
        {
            id = $"d{i + 1}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string> { ["note"] = h.Note, ["category"] = h.Category },
        }))));

    private static Answer Asked(int candidates)
    {
        using var schema = Intake(candidates);
        using var decider = new Decider();
        using (var records = Records())
        {
            decider.Put(schema, records);
        }

        return decider.Answer(schema, "d9", new Dictionary<string, string>
        {
            ["note"] = "router keeps dropping the connection every evening again",
        }).Single();
    }

    private static Candidate[] FromSimilar(Answer answer) =>
        answer.Candidates.Where(c => c.Source == Source.SimilarRecord).ToArray();

    [Fact]
    public void ItIsOffUnlessTheAttributeAsks()
    {
        JsonSerializer.Serialize(new AttributeDefinition("category", AttributeRole.Decided), CoreJson.Default.AttributeDefinition)
            .Should().NotContain("similar_candidates");
        FromSimilar(Asked(0)).Should().HaveCount(1);
        FromSimilar(Asked(1)).Should().HaveCount(1);
    }

    [Fact]
    public void TheValueTheVoteChoseStaysFirstAndTheRunnersUpFollowUntrusted()
    {
        var off = Asked(0);
        var on = Asked(3);
        on.Candidates[0].Should().Be(off.Candidates[0]);
        on.Candidates[0].Trusted.Should().BeTrue();
        var runnersUp = FromSimilar(on).Skip(1).ToArray();
        runnersUp.Select(c => c.Value).Should().Equal("billing", "refund");
        runnersUp.Should().OnlyContain(c => !c.Trusted);
        runnersUp.Select(c => c.Score).Should().BeInDescendingOrder();
        runnersUp.Sum(c => c.Score).Should().BeInRange(0.0, 1.0);
    }

    [Fact]
    public void TheSettingComesBackFromTheCoreAsItWasGivenAndOnlyADecidedSingleValueTakesIt()
    {
        using var on = Intake(3);
        on.Find("category")!.SimilarCandidates.Should().Be(3);
        var refused = () => Schema.FromJson("""
            {"name":"intake","attributes":[{"name":"note","role":"observed","similar_candidates":3}]}
            """);
        refused.Should().Throw<ArgumentException>();
    }
}
