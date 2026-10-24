using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>A new edition of the code lists (experimental): a keeper told to restarts from nothing under it, keeps the
/// new edition alone and leaves records of the edition behind it out.</summary>
public sealed class FormVersionTests
{
    private static readonly string[] Teams = ["network", "billing", "auth"];

    /// <summary>Records <paramref name="from"/> to <paramref name="to"/> under <paramref name="edition"/>; the second
    /// edition renumbers the teams.</summary>
    private static SettledRecords Settled(int from, int to, string edition) => SettledRecords.FromJsonLines(
        string.Join('\n', Enumerable.Range(from, to - from).Select(i => JsonSerializer.Serialize(new
        {
            id = $"t{i}",
            settled_at = (i + 1) * 10_000_000L,
            values = new Dictionary<string, string>
            {
                ["component"] = $"c{i % 7}",
                ["team"] = edition == "2" ? $"{Teams[i % 7 % 3]}-2" : Teams[i % 7 % 3],
            },
            settlements = new Dictionary<string, string> { ["team"] = "Enter" },
            form_version = edition,
        }))));

    private static Schema Plain() => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("team", AttributeRole.Decided),
    ]);

    private static void Settle(Keeper keeper, Decider decider, int from, int to, string edition)
    {
        using var records = Settled(from, to, edition);
        keeper.Settle(decider, records);
    }

    [Fact]
    public void AKeeperToldToRestartKeepsTheNewEditionAlone()
    {
        using var schema = Plain();
        using var keeper = new Keeper(schema, new SelectionOptions(0.5, 5) { RestartOnFormVersion = true });
        using var decider = new Decider();
        Settle(keeper, decider, 0, 200, "1");
        var first = keeper.Rechoose();
        first.FormVersion.Should().Be("1");
        first.Chosen.Should().ContainKey("team");

        Settle(keeper, decider, 200, 300, "2");
        Settle(keeper, decider, 300, 310, "1");
        keeper.Settled.Should().Be(100);
        var state = keeper.State();
        state.FormVersion.Should().Be("2");
        state.Chosen.Should().BeEmpty();
        using (var restarted = keeper.Schema)
        {
            restarted.Find("team")!.KeyThreshold.Should().BeNull();
        }

        keeper.Rechoose().Chosen.Should().ContainKey("team");
    }

    [Fact]
    public void WithoutRestartingEveryEditionIsKept()
    {
        using var schema = Plain();
        using var keeper = new Keeper(schema, new SelectionOptions(0.5, 5));
        using var decider = new Decider();
        Settle(keeper, decider, 0, 100, "1");
        Settle(keeper, decider, 100, 150, "2");
        keeper.Settled.Should().Be(150);
        keeper.State().FormVersion.Should().BeNull();
    }
}
