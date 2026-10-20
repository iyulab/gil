using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

public sealed class PooledRuleTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly string[] Components = ["vpn", "printer", "badge", "laptop", "mail"];
    private static readonly string[] Teams = ["network", "facilities", "security", "hardware", "accounts"];

    private static FormDefinition Ticket(double? keyThreshold = null, double? memoryThreshold = null) => new(
        "ticket",
        [
            new FieldDefinition("component", FieldRole.Observed),
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { SimilarDocumentVotes = 1, KeyThreshold = keyThreshold, MemoryThreshold = memoryThreshold },
        ],
        PromptLanguage.English);

    /// <summary>
    /// Most tickets name a component that decides the team; a share come from a catch-all component whose team varies. Each
    /// band of answers under the catch-all falls short of the target, but taken with the rest the answers hold it.
    /// </summary>
    private static List<SettledDocument> Tickets(int seed, int count, double decided)
    {
        var random = new Random(seed);
        return [.. Enumerable.Range(1, count).Select(i =>
        {
            var (component, team) = random.NextDouble() < decided
                ? (Components[i % Components.Length], Teams[i % Teams.Length])
                : ("other", random.NextDouble() < 0.6 ? "network" : Teams[random.Next(Teams.Length)]);
            var values = new Dictionary<string, string>
            {
                ["component"] = component,
                ["summary"] = $"{component} routine problem report {random.Next(10000, 99999)}",
                ["team"] = team,
            };
            return new SettledDocument($"t{i}", values, DateTimeOffset.UnixEpoch.AddMinutes(i));
        })];
    }

    private static double Lower(double precision, double answered, double z)
    {
        var z2 = z * z;
        return (precision + (z2 / (2 * answered)) - (z * Math.Sqrt((precision * (1 - precision) / answered) + (z2 / (4 * answered * answered))))) / (1 + (z2 / answered));
    }

    private static (int Answered, double Right) Together(LayerThresholds layers)
    {
        var (answered, right) = (0, 0.0);
        foreach (var chosen in new[] { layers.Key.Chosen, layers.Memory.Chosen })
        {
            if (chosen is not null)
            {
                answered += chosen.Answered;
                right += chosen.Precision * chosen.Answered;
            }
        }

        return (answered, right);
    }

    [Fact]
    public async Task The_pooled_rule_answers_more_and_holds_the_target_on_its_answers_together()
    {
        var history = Tickets(seed: 11, count: 600, decided: 0.8);
        var form = Ticket();

        var everyBand = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", history, 0.8, 10, counting: null, ThresholdRule.EveryBand, Ct);
        var pooled = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", history, 0.8, 10, counting: null, ThresholdRule.Pooled, Ct);

        var (answered, right) = Together(pooled);
        answered.Should().BeGreaterThan(Together(everyBand).Answered); // the catch-all's majority team now answers too
        Lower(right / answered, answered, ThresholdSelection.PooledStandardErrors).Should().BeGreaterThanOrEqualTo(0.8);
        pooled.Key.MostPrecise.Should().Be(everyBand.Key.MostPrecise); // how close the layer came does not depend on the rule
    }

    [Fact]
    public async Task Every_band_is_the_rule_of_the_overloads_without_one()
    {
        var history = Tickets(seed: 12, count: 300, decided: 0.85);
        var form = Ticket();

        var without = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", history, 0.8, 10, Ct);
        var with = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", history, 0.8, 10, counting: null, ThresholdRule.EveryBand, Ct);

        with.Key.Chosen.Should().Be(without.Key.Chosen);
        with.Key.MostPrecise.Should().Be(without.Key.MostPrecise);
        with.Memory.Chosen.Should().Be(without.Memory.Chosen);
        with.Memory.MostPrecise.Should().Be(without.Memory.MostPrecise);
    }

    [Fact]
    public async Task The_pooled_rule_reaches_back_when_the_thresholds_as_they_stand_answer_too_little_lately()
    {
        // 2,500 tickets whose component and text say nothing of the team, then 5,000 that all go to one team. The latest
        // 5,000 lookups are answered rightly by either layer; the earlier ones are not.
        var random = new Random(13);
        var history = Enumerable.Range(1, 7_500).Select(i =>
        {
            var team = i <= 2_500 ? Teams[random.Next(Teams.Length)] : "network";
            var values = new Dictionary<string, string>
            {
                ["component"] = "vpn",
                ["summary"] = $"ticket {random.Next(100000, 999999)}",
                ["team"] = team,
            };
            return new SettledDocument($"t{i}", values, DateTimeOffset.UnixEpoch.AddMinutes(i));
        }).ToList();

        // Answering every key lookup as it stands, the latest lookups hold answers enough: the rule chooses on them alone,
        // and its answers over the whole replay fall short.
        var answering = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket(keyThreshold: 0), "team", history, 0.8, 10, counting: null, ThresholdRule.Pooled, Ct);
        // Answering nothing as it stands, the rule reaches back to the first lookup: its answers hold over the whole replay.
        var silent = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket(), "team", history, 0.8, 10, counting: null, ThresholdRule.Pooled, Ct);

        double LowerTogether(LayerThresholds layers)
        {
            var (answered, right) = Together(layers);
            return Lower(right / answered, answered, ThresholdSelection.PooledStandardErrors);
        }

        LowerTogether(answering).Should().BeLessThan(0.8);
        LowerTogether(silent).Should().BeGreaterThanOrEqualTo(0.8);
    }

    [Fact]
    public async Task A_rule_that_is_not_one_is_refused()
    {
        var act = () => ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket(), "team", Tickets(1, 20, 0.9), 0.8, 10, counting: null, (ThresholdRule)7, Ct);
        await act.Should().ThrowAsync<ArgumentOutOfRangeException>();
    }
}
