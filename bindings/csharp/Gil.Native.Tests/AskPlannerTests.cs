using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Native.Tests;

/// <summary>The ask planner from .NET reproduces its golden (<c>core/ask.jsonl</c>): a planner per decided
/// single-valued attribute, advice on every later record with some askable attributes left open — the product every other
/// record, the channel every third, the summary every fifth — before the record is put, learning from it after.</summary>
public sealed class AskPlannerTests
{
    private static readonly string[] Askable = ["product", "channel", "summary"];

    private static bool Open(string attribute, int i) =>
        (attribute == "product" && i % 2 == 0) || (attribute == "channel" && i % 3 == 1) || (attribute == "summary" && i % 5 == 0);

    [Fact]
    public void The_ask_planner_reproduces_its_golden()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        var expected = File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "core", "ask.jsonl"))
            .Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(l => JsonNode.Parse(l)!)
            .ToArray();
        var planners = schema.Attributes
            .Where(a => a.Role == AttributeRole.Decided && !a.Multiple)
            .Select(a => new AskPlanner(schema, a.Name, Askable, 0.3))
            .ToList();
        var observed = CoreTests.Observed();

        var at = 0;
        var lines = CoreTests.RecordLines();
        for (var i = 0; i < lines.Length - 200; i++)
        {
            var record = JsonSerializer.Deserialize(lines[200 + i], CoreJson.Default.Record)!;
            var values = record.Values
                .Where(p => observed.Contains(p.Key) && !Open(p.Key, i))
                .ToDictionary(p => p.Key, p => p.Value);
            var answers = decider.Answer(schema, record.Id, values);
            foreach (var planner in planners)
            {
                var answer = answers.Single(a => a.Attribute == planner.Attribute);
                var actual = new JsonObject
                {
                    ["kind"] = "ask",
                    ["doc"] = record.Id,
                    ["field"] = planner.Attribute,
                    ["answered"] = answer.Answered,
                    ["coarse"] = answer.Coarse is { } c ? new JsonArray(c.Value, c.Trusted) : null,
                    ["advice"] = new JsonArray([.. planner.Advise(answer, values)
                        .Select(a => (JsonNode)new JsonArray(a.Attribute, a.Gain, a.Ask, a.Threshold))]),
                };
                JsonNode.DeepEquals(CoreTests.Doubles(actual), CoreTests.Doubles(expected[at])).Should()
                    .BeTrue($"ask line {at + 1}: {actual.ToJsonString()} vs {expected[at].ToJsonString()}");
                at++;
            }

            using var settled = SettledRecords.From([record]);
            decider.Put(schema, settled);
            foreach (var planner in planners)
            {
                planner.Learn(decider, settled);
            }
        }

        at.Should().Be(expected.Length);
        planners.ForEach(p => p.Dispose());
    }

    [Fact]
    public void A_planner_refuses_what_the_core_refuses()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;

        FluentActions.Invoking(() => new AskPlanner(schema, "team", ["product"], 0))
            .Should().Throw<ArgumentException>().WithMessage("*budget*");
        FluentActions.Invoking(() => new AskPlanner(schema, "team", ["code"], 0.3))
            .Should().Throw<ArgumentException>().WithMessage("*observed*");
        using var team = new AskPlanner(schema, "team", ["product"], 0.3);
        team.Askable.Should().Equal("product");
        var code = decider.Answer(schema, "x", new Dictionary<string, string>()).Single(a => a.Attribute == "code");
        team.Invoking(t => t.Advise(code, new Dictionary<string, string>()))
            .Should().Throw<ArgumentException>().WithMessage("the answer is for 'code', not 'team'");
    }
}
