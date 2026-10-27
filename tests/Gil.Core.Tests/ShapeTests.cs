using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Schemas and records as typed values: a schema built from attributes reads back as the core keeps it, with
/// the defaults filled; one that does not check is refused; records keep their time to the tick across the boundary.
/// </summary>
public sealed class ShapeTests
{
    private static Schema Ticket() => new("ticket", [
        new AttributeDefinition("channel", AttributeRole.Observed),
        new AttributeDefinition("summary", AttributeRole.Observed) { UseAsEvidence = true },
        new AttributeDefinition("team", AttributeRole.Decided)
        {
            KeyThreshold = 0.8,
            MemoryThreshold = 0.6,
            Policy = AttributePolicy.ConfirmRequired,
            Coarse = new CoarseLevel { Prefix = 2, KeyThreshold = 0.9 },
            KeyConcentrations = new Dictionary<string, double> { ["channel"] = 0.5 },
            TypedKeyThresholds = [0.9, null, 0.7],
        },
        new AttributeDefinition("tags", AttributeRole.Decided) { Multiple = true, Candidates = ["refund", "outage"] },
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
            new AttributeDefinition("team", AttributeRole.Decided),
            new AttributeDefinition("team", AttributeRole.Observed),
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

    [Fact]
    public void Settling_with_the_answer_in_view_keeps_it_on_the_record()
    {
        using var schema = new Schema("ticket",
        [
            new AttributeDefinition("channel", AttributeRole.Observed),
            new AttributeDefinition("team", AttributeRole.Decided) { KeyThreshold = 0.5 },
        ]);
        using var decider = new Decider();
        using (var history = SettledRecords.From(Enumerable.Range(1, 4).Select(i => new Record($"h{i}", DateTimeOffset.UnixEpoch)
        {
            Values = new Dictionary<string, string> { ["channel"] = "phone", ["team"] = "network" },
        })))
        {
            decider.Put(schema, history);
        }

        using var ticket = OpenRecord.Open(schema, "t-1", DateTimeOffset.UnixEpoch);
        var team = ticket.Observe(decider, "channel", "phone").Single();
        team.Answered.Should().BeTrue();
        ticket.Settle(decider, "team", SettlementKind.Accept, DateTimeOffset.UnixEpoch, value: "network", shown: team, promise: 0.9);
        var saved = ticket.Snapshot();
        saved.Suggested.Should().Equal(new Dictionary<string, string> { ["team"] = "network" });
        saved.Sources.Should().Equal(new Dictionary<string, Source> { ["team"] = team.Source });
        saved.Promises.Should().Equal(new Dictionary<string, double> { ["team"] = 0.9 });

        ticket.Settle(decider, "team", SettlementKind.CorrectOther, DateTimeOffset.UnixEpoch, value: "it", suggested: "facilities");
        ticket.Snapshot().Suggested.Should().Equal(new Dictionary<string, string> { ["team"] = "facilities" });
        ticket.Invoking(t => t.Settle(decider, "team", SettlementKind.Accept, DateTimeOffset.UnixEpoch, value: "network", shown: team, promise: 1.5))
            .Should().Throw<ArgumentException>();
    }

    [Fact]
    public void A_reopened_record_carries_back_what_the_host_kept_on_it()
    {
        using var schema = Ticket();
        var saved = new Record("t-1", DateTimeOffset.UnixEpoch)
        {
            Values = new Dictionary<string, string> { ["summary"] = "card charged twice", ["team"] = "billing" },
            Settlements = new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Accept },
            Batch = "import-7",
            Suggested = new Dictionary<string, string> { ["team"] = "billing" },
            Sources = new Dictionary<string, Source> { ["team"] = Source.KeyMemory },
            Promises = new Dictionary<string, double> { ["team"] = 0.9 },
            SettledBy = new Dictionary<string, string> { ["team"] = "p7" },
            FormVersion = "2026",
        };

        using var record = OpenRecord.Reopen(schema, saved);
        var back = record.Snapshot();
        (back.Batch, back.FormVersion).Should().Be(("import-7", "2026"));
        back.Suggested.Should().Equal(saved.Suggested);
        back.Sources.Should().Equal(saved.Sources);
        back.Promises.Should().Equal(saved.Promises);
        back.SettledBy.Should().Equal(saved.SettledBy);
        using var ledger = SettledRecords.From([back]);
        Usage.Count(ledger)["team"].Accepted.Should().Be(1);
    }

    [Fact]
    public void A_rejected_settlement_kept_on_a_record_counts_as_turned_down_like_one_left_unsettled()
    {
        var inView = new Dictionary<string, string> { ["team"] = "billing" };
        var layer = new Dictionary<string, Source> { ["team"] = Source.KeyMemory };
        var promise = new Dictionary<string, double> { ["team"] = 0.9 };
        var rejected = new Record("t-2", DateTimeOffset.UnixEpoch)
        {
            Settlements = new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Reject },
            Suggested = inView,
            Sources = layer,
            Promises = promise,
        };
        var left = new Record("t-3", DateTimeOffset.UnixEpoch) { Suggested = inView, Sources = layer, Promises = promise };

        using var ledger = SettledRecords.From([rejected, left]);
        var team = Usage.Count(ledger)["team"];
        (team.Decisions, team.Answered, team.Rejected).Should().Be((2, 2, 2));
    }

    [Fact]
    public void A_coarse_level_is_made_from_a_prefix_or_a_table_and_names_a_value_s_class_as_the_core_does()
    {
        new CoarseLevel(2) { KeyThreshold = 0.9 }.Should().Be(new CoarseLevel { Prefix = 2, KeyThreshold = 0.9 });
        var table = new Dictionary<string, string> { ["vpn"] = "network" };
        new CoarseLevel(table).Classes.Should().BeSameAs(table);

        // The core's own cases (schema.rs): scalars, not UTF-16 units.
        new CoarseLevel(2).Of("25-40").Should().Be("25");
        new CoarseLevel(2).Of("가나다").Should().Be("가나");
        new CoarseLevel(9).Of("short").Should().Be("short");
        new CoarseLevel(1).Of("😀a").Should().Be("😀");
        new CoarseLevel(table).Of("vpn").Should().Be("network");
        new CoarseLevel(table).Of("printer").Should().Be("printer");
    }
}
