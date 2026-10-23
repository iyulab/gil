using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>The first record of a batch (experimental): a keeper chooses and promises the first records apart, the
/// schema and the tuning carry their thresholds, and an open record of a batch is answered at them until a batch mate
/// is settled.</summary>
public sealed class FirstOfBatchTests
{
    private static readonly string[] Teams = ["network", "billing", "auth"];

    private static SettledRecords Settled(int count, int size) => SettledRecords.FromJsonLines(string.Join('\n',
        Enumerable.Range(0, count).Select(i => JsonSerializer.Serialize(new
        {
            id = $"t{i}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string> { ["component"] = $"c{i % 7}", ["team"] = Teams[i % 7 % 3] },
            settlements = new Dictionary<string, string> { ["team"] = "Enter" },
            batch = $"b{i / size}",
        }))));

    private static Schema Plain() => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("team", AttributeRole.Decided),
    ]);

    [Fact]
    public void AKeeperChoosesAndPromisesTheFirstRecordsApart()
    {
        using var schema = Plain();
        using var keeper = new Keeper(schema, new SelectionOptions(0.5, 5) { FirstOfBatch = true });
        using var decider = new Decider();
        using (var records = Settled(400, 4))
        {
            keeper.Settle(decider, records);
        }

        var choice = keeper.Rechoose();
        var first = choice.FirstOfBatch["team"];
        first.Key.Chosen.Should().NotBeNull();
        var tuned = choice.Tuning.Attributes.Single(a => a.Attribute == "team");
        tuned.FirstOfBatch.Should().BeTrue();
        tuned.FirstKeyThreshold.Should().Be(first.Key.Chosen!.Threshold);
        choice.Tuning.FirstOfBatchPromiseOf("team", Source.KeyMemory).Should().Be(first.Key.Chosen.Promise);

        using var chosen = keeper.Schema;
        chosen.Find("team")!.FirstOfBatch!.KeyThreshold.Should().Be(first.Key.Chosen.Threshold);
        using var view = chosen.ForFirstOfBatch();
        view.Find("team")!.KeyThreshold.Should().Be(first.Key.Chosen.Threshold);
        using var with = schema.WithFirstOfBatch("team", first);
        with.Find("team")!.FirstOfBatch.Should().Be(chosen.Find("team")!.FirstOfBatch);

        var byHand = Tuning.Capture(chosen, keeper.Settled, 0.5).PromiseFirstOfBatch("team", first);
        byHand.FirstOfBatchPromiseOf("team", Source.KeyMemory).Should().Be(first.Key.Chosen.Promise);
        Tuning.FromJson(byHand.ToJson()).Should().BeEquivalentTo(byHand);

        using var alike = new Keeper(schema, new SelectionOptions(0.5, 5));
        using var other = new Decider();
        using (var records = Settled(400, 4))
        {
            alike.Settle(other, records);
        }

        alike.Rechoose().FirstOfBatch.Should().BeEmpty();
    }

    [Fact]
    public void AnOpenRecordIsAnsweredAsTheFirstOfItsBatchUntilAMateIsSettled()
    {
        using var schema = new Schema("ticket", [
            new AttributeDefinition("component", AttributeRole.Observed),
            new AttributeDefinition("team", AttributeRole.Decided) { FirstOfBatch = new FirstOfBatch { KeyThreshold = 0 } },
        ]);
        using var decider = new Decider();
        using (var records = Settled(3, 1))
        {
            decider.Put(schema, records);
        }

        decider.IsFirstOfBatch("b9").Should().BeTrue();
        decider.IsFirstOfBatch("b1").Should().BeFalse();
        decider.IsFirstOfBatch("b1", excluding: "t1").Should().BeTrue();
        decider.IsFirstOfBatch(null).Should().BeTrue();

        var at = DateTimeOffset.UnixEpoch.AddMinutes(9);
        using var first = OpenRecord.Open(schema, "n1", at, "b9");
        first.IsFirstOfBatch(decider).Should().BeTrue();
        first.Observe(decider, "component", "c1").Single().Answered.Should().BeTrue();
        first.Snapshot().Batch.Should().Be("b9");

        using var later = OpenRecord.Open(schema, "n2", at, "b1");
        later.IsFirstOfBatch(decider).Should().BeFalse();
        later.Observe(decider, "component", "c1").Single().Answered.Should().BeFalse();
    }
}
