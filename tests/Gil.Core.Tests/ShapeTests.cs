using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Schemas and records as typed values: a schema built from attributes reads back as the core keeps it, with
/// the defaults filled; one that does not check is refused; records keep their time to the tick across the boundary.
/// </summary>
public sealed class ShapeTests
{
    private static Schema Ticket() => new("ticket", [
        new Attribute("channel", AttributeRole.Observed),
        new Attribute("summary", AttributeRole.Observed) { UseAsEvidence = true },
        new Attribute("team", AttributeRole.Decided)
        {
            KeyThreshold = 0.8,
            MemoryThreshold = 0.6,
            Policy = AttributePolicy.ConfirmRequired,
            Coarse = new CoarseLevel { Prefix = 2, KeyThreshold = 0.9 },
            KeyConcentrations = new Dictionary<string, double> { ["channel"] = 0.5 },
            TypedKeyThresholds = [0.9, null, 0.7],
        },
        new Attribute("tags", AttributeRole.Decided) { Multiple = true, Candidates = ["refund", "outage"] },
    ]);

    [Fact]
    public void A_schema_built_from_attributes_reads_back_as_the_core_keeps_it()
    {
        using var schema = Ticket();

        schema.Name.Should().Be("ticket");
        schema.Attributes.Select(a => a.Name).Should().Equal("channel", "summary", "team", "tags");
        var team = schema.Find("team")!;
        team.Role.Should().Be(AttributeRole.Decided);
        team.Policy.Should().Be(AttributePolicy.ConfirmRequired);
        team.SimilarVotes.Should().Be(10, "a setting left out takes the core's default");
        team.UseAsEvidence.Should().BeTrue();
        team.Coarse.Should().Be(new CoarseLevel { Prefix = 2, KeyThreshold = 0.9 });
        team.KeyConcentrations.Should().Equal(new Dictionary<string, double> { ["channel"] = 0.5 });
        team.TypedKeyThresholds.Should().Equal(0.9, null, 0.7);
        schema.Find("tags")!.Candidates.Should().Equal("refund", "outage");
        schema.Find("missing").Should().BeNull();

        var json = JsonNode.Parse(schema.ToJson())!;
        ((string)json["attributes"]![2]!["policy"]!).Should().Be("confirm_required");
        ((string)json["attributes"]![0]!["role"]!).Should().Be("observed");
        using var again = Schema.FromJson(schema.ToJson());
        again.ToJson().Should().Be(schema.ToJson());
    }

    [Fact]
    public void A_schema_that_does_not_check_is_refused()
    {
        var build = () => new Schema("ticket", [
            new Attribute("team", AttributeRole.Decided),
            new Attribute("team", AttributeRole.Observed),
        ]);

        build.Should().Throw<ArgumentException>().Which.Message.Should().Contain("team");
    }

    [Fact]
    public void A_record_keeps_its_time_to_the_tick_and_its_values_across_the_boundary()
    {
        using var schema = Ticket();
        using var decider = new Decider();
        var at = new DateTimeOffset(2026, 10, 21, 9, 30, 15, TimeSpan.FromHours(9)).AddTicks(1234567);
        var settled = new Record("t-1", at)
        {
            Values = new Dictionary<string, string> { ["channel"] = "web", ["summary"] = "card charged twice", ["team"] = "billing" },
            Sets = new Dictionary<string, IReadOnlyList<string>> { ["tags"] = ["refund"] },
            Settlements = new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Correct },
        };
        using (var records = SettledRecords.From([settled, settled with { Id = "t-2", SettledAt = at.AddMinutes(1) }]))
        {
            records.Count.Should().Be(2);
            decider.Put(schema, records);
        }

        using var record = OpenRecord.Reopen(schema, settled);
        var back = record.Snapshot();
        back.SettledAt.Should().Be(at);
        back.SettledAt.UtcTicks.Should().Be(at.UtcTicks);
        back.Values.Should().Equal(settled.Values);
        back.Sets["tags"].Should().Equal("refund");
        back.Settlements.Should().Equal(settled.Settlements);
        back.Arrival.Should().NotBeNull("an open record knows the order its values arrived in");

        record.Invoking(r => r.AnswerPartial(decider, "team")).Should().Throw<ArgumentException>("its team is settled");

        using var fresh = OpenRecord.Open(schema, "t-3", at.AddMinutes(2));
        fresh.Observe(decider, "channel", "web");
        fresh.AnswerPartial(decider, "team").Candidates.Select(c => c.Value).Should()
            .Contain("billing", "the memories hold the two records");
    }
}
