using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>History loaded in bulk (experimental): the records of named batches are replayed into memory but not counted,
/// by a selection and by a keeper alike.</summary>
public sealed class UncountedBatchesTests
{
    private static readonly string[] Teams = ["network", "billing", "auth"];

    private static SettledRecords Settled(int count, int imported) => SettledRecords.FromJsonLines(string.Join('\n',
        Enumerable.Range(0, count).Select(i => JsonSerializer.Serialize(new
        {
            id = $"t{i}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string> { ["component"] = $"c{i % 7}", ["team"] = Teams[i % 7 % 3] },
            settlements = new Dictionary<string, string> { ["team"] = "Enter" },
            batch = i < imported ? "import" : $"live{i}",
        }))));

    private static Schema Plain() => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("team", AttributeRole.Decided),
    ]);

    [Fact]
    public void AnUncountedBatchIsReplayedButNotAskedAbout()
    {
        using var schema = Plain();
        using var records = Settled(400, 200);
        var counted = Replay.SelectLayers(schema, "team", records, new(0.5, 5)).Thresholds;
        var leftOut = Replay.SelectLayers(schema, "team", records, new(0.5, 5) { UncountedBatches = ["import"] }).Thresholds;

        leftOut.Key.Lookups.Should().BeLessThan(counted.Key.Lookups);
        leftOut.Key.Lookups.Should().BeGreaterThan(0, "the records settled since the import are counted");

        // None, empty, or another batch's name chooses as before.
        foreach (IReadOnlyList<string>? same in new IReadOnlyList<string>?[] { null, [], ["elsewhere"] })
        {
            var again = Replay.SelectLayers(schema, "team", records, new(0.5, 5) { UncountedBatches = same }).Thresholds;
            JsonSerializer.Serialize(again, CoreJson.Default.LayerThresholds)
                .Should().Be(JsonSerializer.Serialize(counted, CoreJson.Default.LayerThresholds));
        }
    }

    [Fact]
    public void AKeeperLeavesThemOutAsASelectionDoes()
    {
        using var schema = Plain();
        var options = new SelectionOptions(0.5, 5) { UncountedBatches = ["import"] };
        using var keeper = new Keeper(schema, options);
        using var decider = new Decider();
        using (var records = Settled(400, 200))
        {
            keeper.Settle(decider, records);
        }

        var chosen = keeper.Rechoose().Chosen["team"];
        using var all = Settled(400, 200);
        var alone = Replay.SelectLayers(schema, "team", all, options).Thresholds;
        JsonSerializer.Serialize(chosen, CoreJson.Default.LayerThresholds)
            .Should().Be(JsonSerializer.Serialize(alone, CoreJson.Default.LayerThresholds));
    }
}
