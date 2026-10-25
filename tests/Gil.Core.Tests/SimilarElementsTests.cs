using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Similar elements (experimental): the similar record layer answering an attribute that takes several values,
/// element by element, by the share of the nearest records that hold each.</summary>
public sealed class SimilarElementsTests
{
    private static Schema Desk(bool elements) => new("desk", [
        new AttributeDefinition("text", AttributeRole.Observed),
        new AttributeDefinition("tags", AttributeRole.Decided) { Multiple = true, SimilarElements = elements },
    ]);

    /// <summary>Texts that name their topic among noise words; a topic's two tags are always given.</summary>
    private static SettledRecords Records() => SettledRecords.FromJsonLines(string.Join('\n',
        Enumerable.Range(0, 240).Select(i => JsonSerializer.Serialize(new
        {
            id = $"r{i:000}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string> { ["text"] = $"topic{i % 4}x w{i * 7919 % 97} topic{i % 4}y" },
            sets = new Dictionary<string, string[]> { ["tags"] = [$"core{i % 4}a", $"core{i % 4}b"] },
        }))));

    private static readonly Dictionary<string, string> Asking = new() { ["text"] = "topic2x w5 topic2y" };

    [Fact]
    public void AKeeperOffersTheElementsTheSimilarRecordsAgreeOnWithThePromise()
    {
        using var schema = Desk(elements: true);
        using var keeper = new Keeper(schema, new SelectionOptions(0.9, 10));
        using var decider = new Decider();
        using (var records = Records())
        {
            keeper.Settle(decider, records);
        }

        keeper.SimilarElements(decider, "asking", "tags", Asking).Should().BeEmpty("no threshold before a choice");

        keeper.Rechoose();
        var tuned = keeper.Tuning.Attributes.Single();
        tuned.MemoryThreshold.Should().NotBeNull();
        tuned.MemoryPromise.Should().NotBeNull();
        var offered = keeper.SimilarElements(decider, "asking", "tags", Asking,
            new Dictionary<string, IReadOnlyList<string>> { ["tags"] = ["core0a"] });
        offered.Select(e => e.Value).Should().Equal("core2a", "core2b");
        offered.Should().OnlyContain(e => e.Share > 0.9 && e.Promise == tuned.MemoryPromise);
        using var chosen = keeper.Schema;
        chosen.Find("tags")!.MemoryThreshold.Should().Be(tuned.MemoryThreshold);
    }

    [Fact]
    public void ItIsOffUnlessTheAttributeAsksAndAskingForAnAttributeThatDoesNotIsRefused()
    {
        JsonSerializer.Serialize(new AttributeDefinition("tags", AttributeRole.Decided), CoreJson.Default.AttributeDefinition)
            .Should().NotContain("similar_elements");
        using var plain = Desk(elements: false);
        using var keeper = new Keeper(plain, new SelectionOptions(0.9, 10));
        using var decider = new Decider();
        using (var records = Records())
        {
            keeper.Settle(decider, records);
        }

        keeper.Rechoose().Chosen.Should().BeEmpty();
        keeper.Tuning.Attributes.Single().MemoryThreshold.Should().BeNull();
        var refused = () => keeper.SimilarElements(decider, "asking", "tags", Asking);
        refused.Should().Throw<ArgumentException>().WithMessage("*not answered by similar elements*");
    }

    [Fact]
    public void TheSettingComesBackFromTheCoreAsItWasGiven()
    {
        using var on = Desk(elements: true);
        on.Find("tags")!.SimilarElements.Should().BeTrue();
        using var read = Schema.FromJson("""
            {"name":"desk","attributes":[
              {"name":"text","role":"observed"},
              {"name":"tags","role":"decided","multiple":true,"similar_elements":true}]}
            """);
        read.Find("tags")!.SimilarElements.Should().BeTrue();
        var refused = () => Schema.FromJson("""
            {"name":"desk","attributes":[{"name":"kind","role":"decided","similar_elements":true}]}
            """);
        refused.Should().Throw<ArgumentException>();
    }
}
