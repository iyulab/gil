using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>How each judged value was settled travels with the saved document, and choosing thresholds can count by it.</summary>
public sealed class SettlementOriginTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly FormDefinition Ticket = new(
        "ticket",
        [
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.3 },
        ],
        PromptLanguage.English);

    private static readonly FormDefinition Tagged = new(
        "tagged",
        [
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("tags", FieldRole.Judged) { Multiple = true },
        ],
        PromptLanguage.English);

    private static readonly DateTimeOffset Monday = new(2026, 9, 28, 9, 0, 0, TimeSpan.Zero);

    private sealed class Clock(DateTimeOffset now) : TimeProvider
    {
        public DateTimeOffset Now { get; set; } = now;

        public override DateTimeOffset GetUtcNow() => Now;
    }

    private static SettledDocument Doc(string id, string summary, string team, int day, SettlementKind? kind) =>
        new(id, new Dictionary<string, string> { ["summary"] = summary, ["team"] = team }, Monday.AddDays(day))
        {
            Settlements = kind is { } k
                ? new Dictionary<string, SettlementKind> { ["team"] = k }
                : new Dictionary<string, SettlementKind>(),
        };

    [Fact]
    public async Task Entering_or_filling_a_field_records_how_it_was_settled_in_the_snapshot()
    {
        var clock = new Clock(Monday);
        var session = new FormResolver(new FieldMemory(), timeProvider: clock).Open(Ticket, "d1", Monday.AddDays(-3));
        await session.ObserveAsync("summary", "vpn drops", Ct);

        await session.SettleAsync("team", Settlement.Enter("network"), Ct);
        session.Snapshot().Settlements.Should().Equal(new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Enter });
        session.Snapshot().SettledAt.Should().Be(Monday); // entering is settling: the time moves on

        clock.Now = Monday.AddDays(1);
        await session.SettleAsync("team", Settlement.Fill("network"), Ct);
        session.Snapshot().Settlements["team"].Should().Be(SettlementKind.Fill);
        session.Snapshot().SettledAt.Should().Be(Monday.AddDays(1));

        await session.SettleAsync("team", Settlement.Correct("facilities"), Ct);
        session.Snapshot().Settlements["team"].Should().Be(SettlementKind.Correct);
        session.Snapshot().Settlements.Should().NotContainKey("summary"); // an observed value is not settled
    }

    [Fact]
    public async Task A_value_settled_over_another_suggestion_is_recorded_as_such_learned_and_counted()
    {
        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        var kept = resolver.Open(Ticket, "d1");
        await kept.ObserveAsync("summary", "vpn drops", Ct);
        await kept.SettleAsync("team", Settlement.AcceptOther("network"), Ct);
        kept.Snapshot().Settlements["team"].Should().Be(SettlementKind.AcceptOther);

        var changed = resolver.Open(Ticket, "d2");
        await changed.ObserveAsync("summary", "printer jams", Ct);
        await changed.SettleAsync("team", Settlement.CorrectOther("facilities"), Ct);
        changed.Snapshot().Settlements["team"].Should().Be(SettlementKind.CorrectOther);

        (await resolver.Open(Ticket, "d3").ObserveAsync("summary", "vpn drops", Ct))
            .Single(s => s.Field == "team").Candidates.Select(c => c.Value).Should().Contain("network"); // learned, unlike a fill

        SettledDocument[] documents =
        [
            Doc("d1", "vpn drops", "network", 0, SettlementKind.AcceptOther),
            Doc("d2", "vpn drops", "network", 1, SettlementKind.CorrectOther),
            Doc("d3", "vpn drops", "network", 2, SettlementKind.AcceptOther),
        ];
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 1.0, 1).Lookups.Should().Be(2);
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 1.0, 1, counting: [SettlementKind.Enter])
            .Lookups.Should().Be(0); // counted only where asked for
    }

    [Fact]
    public async Task Rejecting_reverting_or_restoring_a_field_forgets_how_it_was_settled()
    {
        var session = new FormResolver(new FieldMemory()).Open(Ticket, "d1");
        await session.ObserveAsync("summary", "vpn drops", Ct);

        await session.SettleAsync("team", Settlement.Accept("network"), Ct);
        await session.SettleAsync("team", Settlement.Restore("network"), Ct);
        session.Snapshot().Settlements.Should().BeEmpty(); // a value put back is not known to have been settled any way

        await session.SettleAsync("team", Settlement.Accept("network"), Ct);
        await session.SettleAsync("team", Settlement.Reject(), Ct);
        session.Snapshot().Settlements.Should().BeEmpty();

        await session.SettleAsync("team", Settlement.Enter("network"), Ct);
        await session.SettleAsync("team", Settlement.Revert(), Ct);
        session.Snapshot().Settlements.Should().BeEmpty();
    }

    [Fact]
    public async Task A_field_that_takes_several_values_is_settled_as_a_set_not_entered_or_filled()
    {
        var session = new FormResolver(new FieldMemory()).Open(Tagged, "d1");

        await session.Invoking(s => s.SettleAsync("tags", Settlement.Enter("vpn"), Ct)).Should().ThrowAsync<ArgumentException>();
        await session.Invoking(s => s.SettleAsync("tags", Settlement.Fill("vpn"), Ct)).Should().ThrowAsync<ArgumentException>();

        await session.SettleAsync("tags", Settlement.Set(["vpn", "remote"]), Ct);
        session.Snapshot().Settlements["tags"].Should().Be(SettlementKind.Set);

        await session.SettleAsync("tags", Settlement.Set([]), Ct); // no values left: nothing settled
        session.Snapshot().Settlements.Should().BeEmpty();
    }

    [Fact]
    public void Opening_a_saved_document_brings_back_its_values_arrival_and_settlements()
    {
        var saved = new SettledDocument(
            "d1",
            new Dictionary<string, string> { ["summary"] = "vpn drops", ["team"] = "network" },
            Monday,
            ["team", "summary"]) // the summary was observed after the team was settled
        {
            Settlements = new Dictionary<string, SettlementKind>
            {
                ["team"] = SettlementKind.Enter,
                ["summary"] = SettlementKind.Accept, // not a judged field: nothing to keep
            },
        };

        var reopened = new FormResolver(new FieldMemory(), timeProvider: new Clock(Monday.AddDays(5))).Open(Ticket, saved).Snapshot();

        reopened.DocumentId.Should().Be("d1");
        reopened.Values.Should().Equal(saved.Values);
        reopened.Arrival.Should().Equal("team", "summary");
        reopened.SettledAt.Should().Be(Monday);
        reopened.Settlements.Should().Equal(new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Enter });
    }

    [Fact]
    public void Opening_a_saved_document_without_an_arrival_order_takes_the_observed_values_first()
    {
        var saved = new SettledDocument(
            "d1",
            new Dictionary<string, string> { ["team"] = "network", ["summary"] = "vpn drops" },
            Monday);

        var reopened = new FormResolver(new FieldMemory()).Open(Ticket, saved).Snapshot();

        reopened.Arrival.Should().Equal("summary", "team");
        reopened.Settlements.Should().BeEmpty();
    }

    [Fact]
    public async Task Counting_only_entered_settlements_replays_every_document_but_asks_only_those()
    {
        SettledDocument[] documents =
        [
            Doc("d1", "vpn drops", "network", 0, SettlementKind.Accept),
            Doc("d2", "vpn drops", "network", 1, SettlementKind.Enter),
            Doc("d3", "vpn drops", "network", 2, SettlementKind.Fill),
            Doc("d4", "vpn drops", "facilities", 3, SettlementKind.Enter),
            Doc("d5", "vpn drops", "network", 4, null), // saved before settlements were kept
        ];

        var keys = new FieldMemory();
        var entered = ThresholdSelection.SelectKeyThreshold(keys, Ticket, "team", documents, 0.5, 1, counting: [SettlementKind.Enter]);
        entered.Lookups.Should().Be(2); // d2 and d4 — d1 meets an empty memory, the rest are not counted
        keys.Count("ticket").Should().Be(4); // every document a person settled is still replayed into memory — the filled d3 is not

        var all = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 1);
        all.Lookups.Should().Be(3); // d2, d4 and d5, the one not known included — the filled d3 is never asked about

        var memory = new LexicalMemory();
        var similar = await ThresholdSelection.SelectAsync(memory, Ticket, "team", documents, 0.5, 1, [SettlementKind.Enter, SettlementKind.Accept], Ct);
        similar.Lookups.Should().Be(2);
    }

    [Fact]
    public async Task A_filled_value_stays_in_the_snapshot_but_memory_does_not_learn_it()
    {
        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        var filled = resolver.Open(Ticket, "d1");
        await filled.ObserveAsync("summary", "vpn drops", Ct);
        await filled.SettleAsync("team", Settlement.Fill("network"), Ct);
        filled.Snapshot().Values["team"].Should().Be("network");

        var next = resolver.Open(Ticket, "d2");
        var suggestions = await next.ObserveAsync("summary", "vpn drops", Ct);
        suggestions.Single(s => s.Field == "team").Candidates.Should().BeEmpty(); // nothing a person settled to go on

        var accepted = resolver.Open(Ticket, "d3");
        await accepted.ObserveAsync("summary", "vpn drops", Ct);
        await accepted.SettleAsync("team", Settlement.Accept("network"), Ct);
        (await resolver.Open(Ticket, "d4").ObserveAsync("summary", "vpn drops", Ct))
            .Single(s => s.Field == "team").Candidates.Select(c => c.Value).Should().Contain("network");
    }

    [Fact]
    public async Task A_filled_value_settled_again_as_entered_is_learned_and_counted()
    {
        // An application that fills a field itself can have the value checked later with no suggestion in view, and
        // settle the same field of the same document again with what the check found.
        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        var session = resolver.Open(Ticket, "d1");
        await session.ObserveAsync("summary", "vpn drops", Ct);
        await session.SettleAsync("team", Settlement.Fill("facilities"), Ct);
        await session.SettleAsync("team", Settlement.Enter("network"), Ct);
        session.Snapshot().Settlements["team"].Should().Be(SettlementKind.Enter);

        var candidates = (await resolver.Open(Ticket, "d2").ObserveAsync("summary", "vpn drops", Ct))
            .Single(s => s.Field == "team").Candidates.Select(c => c.Value).ToList();
        candidates.Should().Contain("network").And.NotContain("facilities"); // the checked value replaced the filled one

        SettlementKind[] byHand = [SettlementKind.Accept, SettlementKind.Correct, SettlementKind.Enter];
        SettledDocument[] filled =
        [
            Doc("d1", "vpn drops", "network", 0, SettlementKind.Accept),
            Doc("d2", "vpn drops", "network", 1, SettlementKind.Fill),
            Doc("d3", "vpn drops", "network", 2, SettlementKind.Fill),
        ];
        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", filled, 1.0, 1, counting: byHand)
            .Lookups.Should().Be(0); // nothing the application filled is asked about

        SettledDocument[] checkedLater = [filled[0], filled[1], Doc("d3", "vpn drops", "network", 2, SettlementKind.Enter)];
        var keys = new FieldMemory();
        var replay = ThresholdSelection.SelectKeyThreshold(keys, Ticket, "team", checkedLater, 1.0, 1, counting: byHand);
        replay.Lookups.Should().Be(1); // the checked d3 is asked about, against what d1 taught
        replay.Chosen.Should().NotBeNull();
        keys.Count("ticket").Should().Be(2); // d1 and the checked d3 — the filled d2 is still left out
    }

    [Fact]
    public async Task Choosing_thresholds_neither_asks_about_nor_remembers_a_filled_value()
    {
        SettledDocument[] documents =
        [
            Doc("d1", "vpn drops", "network", 0, SettlementKind.Accept),
            Doc("d2", "vpn drops", "facilities", 1, SettlementKind.Fill),
            Doc("d3", "vpn drops", "facilities", 2, SettlementKind.Fill),
            Doc("d4", "vpn drops", "network", 3, SettlementKind.Accept),
        ];

        // Were the fills remembered, d4 would meet two "facilities" against one "network" and be answered wrongly.
        var keys = new FieldMemory();
        var replay = ThresholdSelection.SelectKeyThreshold(keys, Ticket, "team", documents, 1.0, 1);
        replay.Lookups.Should().Be(1);
        replay.Chosen.Should().NotBeNull();
        keys.Rank(Ticket, "team", new Dictionary<string, string> { ["summary"] = "vpn drops" }, 3).Select(c => c.Value).Should().Equal("network");

        var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket, "team", documents, 1.0, 1, Ct);
        layers.Key.Lookups.Should().Be(1); // the similar document layer is left only what the key did not answer — here nothing
        layers.Memory.Lookups.Should().Be(0);
    }

    [Fact]
    public async Task Counting_nothing_in_particular_replays_as_before()
    {
        SettledDocument[] withKinds =
        [
            Doc("d1", "vpn drops", "network", 0, SettlementKind.Accept),
            Doc("d2", "vpn drops", "facilities", 1, SettlementKind.Enter),
            Doc("d3", "printer jammed", "facilities", 2, SettlementKind.Enter),
            Doc("d4", "vpn drops", "network", 3, SettlementKind.Correct),
        ];
        var withoutKinds = withKinds.Select(d => d with { Settlements = new Dictionary<string, SettlementKind>() }).ToArray();

        ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", withKinds, 0.5, 1)
            .Should().BeEquivalentTo(ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", withoutKinds, 0.5, 1));
        (await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket, "team", withKinds, 0.5, 1, Ct))
            .Should().BeEquivalentTo(await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket, "team", withoutKinds, 0.5, 1, Ct));
    }
}
