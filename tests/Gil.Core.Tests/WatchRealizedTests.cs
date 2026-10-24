using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Watching the realized precision (experimental): a keeper told to says, as a stale reason, which attribute's
/// answers stopped keeping the promise.</summary>
public sealed class WatchRealizedTests
{
    /// <summary>Records <paramref name="from"/> to <paramref name="to"/> whose team answer was accepted on the even ones
    /// and corrected on the odd.</summary>
    private static SettledRecords Judged(int from, int to) => SettledRecords.FromJsonLines(
        string.Join('\n', Enumerable.Range(from, to - from).Select(i => JsonSerializer.Serialize(new
        {
            id = $"t{i}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string> { ["component"] = $"c{i % 7}", ["team"] = "network" },
            settlements = new Dictionary<string, string> { ["team"] = i % 2 == 0 ? "Accept" : "Correct" },
        }))));

    private static Schema Plain() => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("team", AttributeRole.Decided),
    ]);

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void AKeeperToldToWatchNamesTheAttributeWhoseAnswersFellShort(bool watch)
    {
        using var schema = Plain();
        using var keeper = new Keeper(schema, new SelectionOptions(0.9, 5) { WatchRealized = watch });
        using var decider = new Decider();
        using (var records = Judged(0, 100))
        {
            keeper.Settle(decider, records);
        }

        keeper.Rechoose();
        if (watch)
        {
            keeper.Stale().Should().Be(new Stale.Realized("team", 50, 100));
        }
        else
        {
            keeper.Stale().Should().BeNull();
        }
    }

    [Fact]
    public void TheReasonReadsBackAsItWasWritten()
    {
        Stale reason = new Stale.Realized("team", 60, 100);
        var json = JsonSerializer.Serialize(reason, CoreJson.Default.Stale);
        json.Should().Be("""{"realized":{"attribute":"team","right":60,"judged":100}}""");
        JsonSerializer.Deserialize(json, CoreJson.Default.Stale).Should().Be(reason);
    }
}
