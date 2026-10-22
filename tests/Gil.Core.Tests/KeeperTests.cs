using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>The keeper chooses as a replay from the start would, on the records settled so far, and puts them into the
/// decider it is given.</summary>
public sealed class KeeperTests
{
    private static string Fixture(string name) => File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "core", name));

    private static string[] Lines() => Fixture("records.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries);

    private static SettledRecords Of(IEnumerable<string> lines) => SettledRecords.FromJsonLines(string.Join('\n', lines));

    private static string Json(LayerThresholds thresholds) =>
        JsonSerializer.Serialize(thresholds, CoreJson.Default.LayerThresholds);

    [Fact]
    public void RechoosingAsRecordsArriveChoosesAsASelectionFromTheStart()
    {
        using var schema = Schema.FromJson(Fixture("schema.json"));
        var options = new SelectionOptions(0.8, 10) { Rule = ThresholdRule.Pooled };
        var lines = Lines();
        using var keeper = new Keeper(schema, options);
        using var decider = new Decider();
        keeper.Stale().Should().BeNull();
        keeper.State().Chosen.Should().BeEmpty();

        var done = 0;
        foreach (var at in new[] { 120, 121, 300, lines.Length })
        {
            using (var part = Of(lines[done..at]))
            {
                keeper.Settle(decider, part);
            }

            // Stale by the tuning rule: a tenth more records than the last choice — one more than 120 is not.
            (keeper.Stale() is null).Should().Be(at == 121);
            done = at;
            var choice = keeper.Rechoose();
            keeper.Stale().Should().BeNull();
            keeper.Settled.Should().Be(at);
            choice.Chosen.Keys.Should().BeEquivalentTo(["team", "code"]);
            using var all = Of(lines[..at]);
            foreach (var attribute in choice.Chosen.Keys)
            {
                Json(choice.Chosen[attribute]).Should().Be(
                    Json(Replay.SelectLayers(schema, attribute, all, options).Thresholds), $"{attribute} at {at}");
            }
        }

        // The schema carries the choice, and the tuning its promises.
        var team = keeper.State().Chosen["team"];
        using (var chosen = keeper.Schema)
        {
            chosen.Attributes.Single(a => a.Name == "team").KeyThreshold.Should().Be(team.Key.Chosen?.Threshold);
        }

        keeper.Tuning.PromiseOf("team", Source.KeyMemory).Should().Be(team.Key.Chosen?.Promise);
    }

    [Fact]
    public void TheDeciderSettledThroughTheKeeperAnswersAsOnePut()
    {
        using var schema = Schema.FromJson(Fixture("schema.json"));
        var lines = Lines();
        using var keeper = new Keeper(schema, new SelectionOptions(0.8, 10) { Threads = 1 });
        using var through = new Decider();
        using var put = new Decider();
        using (var records = Of(lines))
        {
            keeper.Settle(through, records);
            put.Put(schema, records);
        }

        var removed = JsonDocument.Parse(lines[5]).RootElement.GetProperty("id").GetString()!;
        keeper.Remove(through, removed);
        put.Remove(schema, removed);
        var choice = keeper.Rechoose(decisions: true);
        choice.Decisions.Should().NotBeNullOrEmpty();
        using var chosen = keeper.Schema;
        var values = new Dictionary<string, string> { ["product"] = "vpn", ["summary"] = "drops every hour" };
        JsonSerializer.Serialize(through.Answer(chosen, "new", values), CoreJson.Default.IReadOnlyListAnswer)
            .Should().Be(JsonSerializer.Serialize(put.Answer(chosen, "new", values), CoreJson.Default.IReadOnlyListAnswer));
    }
}
