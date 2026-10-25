using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>The unseen mass (experimental): a keeper says how much of an attribute's next settled values is likely to be
/// values no record settled yet.</summary>
public sealed class UnseenMassTests
{
    private static Schema Plain() => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("team", AttributeRole.Decided),
    ]);

    [Fact]
    public void AKeeperSaysHowManyValuesWereSettledOnce()
    {
        using var schema = Plain();
        using var keeper = new Keeper(schema, new SelectionOptions(0.5, 5));
        using var decider = new Decider();
        keeper.UnseenMass("team").Should().BeNull();
        string[] teams = ["network", "network", "billing", "auth"];
        using (var records = SettledRecords.FromJsonLines(string.Join('\n', teams.Select((team, i) =>
                   JsonSerializer.Serialize(new
                   {
                       id = $"t{i}",
                       settled_at = i + 1L,
                       values = new Dictionary<string, string> { ["component"] = "c", ["team"] = team },
                   })))))
        {
            keeper.Settle(decider, records);
        }

        keeper.UnseenMass("team").Should().Be(new UnseenMass(2, 4, 0.5));
        keeper.UnseenMass("component").Should().BeNull();
    }
}
