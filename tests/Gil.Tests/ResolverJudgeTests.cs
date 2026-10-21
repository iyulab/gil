using AwesomeAssertions;
using Gil.Fallback;
using Gil.Memory;
using Gil.Tree;

namespace Gil.Tests;

public sealed class ResolverJudgeTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static Schema Ticket() => new(
        "ticket",
        [
            new Attribute("reporter", AttributeRole.Observed) { UseAsEvidence = false },
            new Attribute("component", AttributeRole.Observed),
            new Attribute("summary", AttributeRole.Observed),
            new Attribute("team", AttributeRole.Decided) { KeyThreshold = 0.5 },
        ]);

    private static readonly Attribute Team = new("team", AttributeRole.Decided);

    /// <summary>A task for <c>team</c> whose bare tree never judges: only the resolver's memory answers it.</summary>
    private static TaskDefinition TeamTask() => new(
        "ticket/team",
        new TextContract(),
        TreeYaml.Parse("id: root").Root,
        new TaskPolicy { Thresholds = new([1.0], 1.0), MemoryThreshold = 0.5 },
        PromptLanguage.English);

    private static ResolverJudge Judge(IMemory memory, ITelemetrySink sink) =>
        new(new Resolver(memory, sink), new Dictionary<string, TaskDefinition> { ["team"] = TeamTask() });

    private static Record Settled(string id, string component, string team) =>
        new(id, DateTimeOffset.UnixEpoch.AddMinutes(int.Parse(id[1..], System.Globalization.CultureInfo.InvariantCulture)))
        {
            Values = new Dictionary<string, string> { ["component"] = component, ["team"] = team },
        };

    [Fact]
    public async Task The_request_is_the_evidence_as_lines_and_the_resolver_records_it_in_its_own_sink()
    {
        var sink = new ListSink();
        var answers = new LexicalMemory();
        await answers.RememberAsync("ticket/team", "earlier", "summary: badge reader is broken", "security", "t0", Ct);

        var ranked = await Judge(answers, sink).JudgeAsync(Team, [new("summary", "the badge reader is broken")], Ct);

        ranked.Should().ContainSingle().Which.Should().Match<ExternalCandidate>(c => c.Value == "security" && c.Trusted);
        var trace = sink.Traces.Should().ContainSingle().Subject.Value;
        (trace.Task, trace.State, trace.Outcome!.Mode).Should().Be(("ticket/team", "summary: the badge reader is broken", "memory"));
    }

    [Fact]
    public async Task An_attribute_without_a_task_abstains_without_a_request()
    {
        var sink = new ListSink();
        var judge = new ResolverJudge(new Resolver(new LexicalMemory(), sink), new Dictionary<string, TaskDefinition>());

        var ranked = await judge.JudgeAsync(Team, [new("summary", "x")], Ct);

        ranked.Should().BeEmpty();
        sink.Traces.Should().BeEmpty();
    }

    [Fact]
    public async Task A_request_the_resolver_leaves_without_an_output_abstains()
    {
        var sink = new ListSink();

        var ranked = await Judge(new LexicalMemory(), sink).JudgeAsync(Team, [new("summary", "nothing like it")], Ct);

        ranked.Should().BeEmpty();
        sink.Traces.Should().ContainSingle().Which.Value.Outcome!.Mode.Should().Be("abstain");
    }

    [Fact]
    public async Task An_open_record_gives_the_request_its_evidence_in_arrival_order_and_a_changed_value_moves_to_the_end()
    {
        using var schema = Ticket();
        using var decider = new Decider();
        using var record = OpenRecord.Open(schema, "d1", DateTimeOffset.UnixEpoch);
        var sink = new ListSink();
        var judge = Judge(new LexicalMemory(), sink);

        record.Observe(decider, "summary", "door will not open");
        record.Observe(decider, "component", "badge reader");
        record.Observe(decider, "summary", "door opens late");
        await record.AnswerAsync(decider, judge, Ct);

        sink.Traces.Should().ContainSingle().Which.Value.State.Should().Be("component: badge reader\nsummary: door opens late");
    }

    [Fact]
    public async Task An_open_record_asks_the_resolver_only_where_no_memory_has_evidence_and_its_value_leads_the_answer()
    {
        using var schema = Ticket();
        using var decider = new Decider();
        using (var settled = SettledRecords.From([Settled("d1", "vpn", "network"), Settled("d2", "vpn", "network")]))
        {
            decider.Put(schema, settled);
        }

        var sink = new ListSink();
        var answers = new LexicalMemory();
        await answers.RememberAsync("ticket/team", "earlier", "component: badge reader", "security", "t0", Ct);
        var judge = Judge(answers, sink);

        // A known key: the key memory answers and the resolver is not asked.
        using var keyed = OpenRecord.Open(schema, "d3", DateTimeOffset.UnixEpoch.AddMinutes(3));
        keyed.Observe(decider, "component", "vpn");
        var known = (await keyed.AnswerAsync(decider, judge, Ct)).Single();
        (known.Source, known.Candidates[0].Value, known.Answered).Should().Be((Source.SettledFieldMemory, "network", true));
        sink.Traces.Should().BeEmpty();

        // No key matches: the resolver's value leads; the reporter never reaches it.
        using var unknown = OpenRecord.Open(schema, "d4", DateTimeOffset.UnixEpoch.AddMinutes(4));
        unknown.Observe(decider, "reporter", "Kim");
        unknown.Observe(decider, "component", "badge reader");
        var judged = (await unknown.AnswerAsync(decider, judge, Ct)).Single();
        (judged.Source, judged.Candidates[0].Value, judged.Candidates[0].Source).Should().Be((Source.External, "security", Source.External));
        sink.Traces.Should().ContainSingle().Which.Value.State.Should().Be("component: badge reader");
    }
}
