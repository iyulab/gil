using System.Text.Json;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Key combination (experimental): an attribute that takes its strongest key alone answers as before when another
/// attribute repeats one of its keys, and the setting comes back from the core as it was given.</summary>
public sealed class KeyCombineTests
{
    /// <summary>Tickets whose team follows the component most of the time; with <paramref name="repeated"/>, a module
    /// attribute that copies the component.</summary>
    private static SettledRecords Tickets(bool repeated) => SettledRecords.FromJsonLines(
        string.Join('\n', Enumerable.Range(0, 200).Select(i =>
        {
            var values = new Dictionary<string, string>
            {
                ["component"] = $"c{i % 7}",
                ["channel"] = $"h{i % 3}",
                ["team"] = i % 5 == 0 ? $"t{i % 3}" : $"t{i % 7 % 4}",
            };
            if (repeated)
            {
                values["module"] = values["component"];
            }

            return JsonSerializer.Serialize(new { id = $"t{i}", settled_at = (i + 1) * 10_000_000L, values });
        })));

    private static Schema Tickets(KeyCombine combine, bool repeated) => new("ticket", [
        new AttributeDefinition("component", AttributeRole.Observed),
        new AttributeDefinition("channel", AttributeRole.Observed),
        .. repeated ? [new AttributeDefinition("module", AttributeRole.Observed)] : Array.Empty<AttributeDefinition>(),
        new AttributeDefinition("team", AttributeRole.Decided) { KeyCombine = combine },
    ]);

    private static (string Value, double Score)[] Ranked(KeyCombine combine, bool repeated)
    {
        using var schema = Tickets(combine, repeated);
        using var decider = new Decider();
        using (var records = Tickets(repeated))
        {
            decider.Put(schema, records);
        }

        var values = new Dictionary<string, string> { ["component"] = "c3", ["channel"] = "h1" };
        if (repeated)
        {
            values["module"] = "c3";
        }

        return [.. decider.Answer(schema, "new", values).Single().Candidates.Select(c => (c.Value, c.Score))];
    }

    [Fact]
    public void TakingTheStrongestAnswersAsBeforeWhenAnAttributeRepeatsAKey()
    {
        var alone = Ranked(KeyCombine.Max, repeated: false);
        alone.Should().NotBeEmpty();
        Ranked(KeyCombine.Max, repeated: true).Should().Equal(alone);
        Ranked(KeyCombine.Sum, repeated: true).Should().NotEqual(Ranked(KeyCombine.Sum, repeated: false));
    }

    [Fact]
    public void TheSettingComesBackFromTheCoreAndSummingWritesNone()
    {
        using var strongest = Tickets(KeyCombine.Max, repeated: false);
        strongest.Find("team")!.KeyCombine.Should().Be(KeyCombine.Max);
        using var read = Schema.FromJson("""
            {"name":"ticket","attributes":[
              {"name":"component","role":"observed"},
              {"name":"team","role":"decided","key_combine":"max"}]}
            """);
        read.Find("team")!.KeyCombine.Should().Be(KeyCombine.Max);

        JsonSerializer.Serialize(new AttributeDefinition("team", AttributeRole.Decided), CoreJson.Default.AttributeDefinition)
            .Should().NotContain("key_combine");
    }
}
