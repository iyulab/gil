using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>Answers as typed values: everything the core writes — candidates with what backs them, the coarse class, the
/// values an external judgement would read, the decision record — reads into them and writes back as the same JSON; open
/// records hand back answers from every call that causes them.</summary>
public sealed class AnswerTests
{
    [Fact]
    public void Typed_answers_hold_everything_the_core_writes()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        var observed = CoreTests.Observed();
        var (evidence, decisions, coarse) = (0, 0, 0);
        foreach (var line in CoreTests.RecordLines()[200..260])
        {
            var log = JsonNode.Parse(line)!;
            var values = log["values"]!.AsObject()
                .Where(p => observed.Contains(p.Key))
                .ToDictionary(p => p.Key, p => (string)p.Value!);
            var json = decider.AnswerJson(schema, (string)log["id"]!, values, decisions: true);
            var typed = decider.Answer(schema, (string)log["id"]!, values, decisions: true);

            var written = JsonNode.Parse(JsonSerializer.Serialize(typed, CoreJson.Default.IReadOnlyListAnswer));
            JsonNode.DeepEquals(Comparable(written), Comparable(JsonNode.Parse(json))).Should()
                .BeTrue($"{written!.ToJsonString()} vs {json}");
            evidence += typed.Sum(a => a.Candidates.Count(c => c.Evidence is not null));
            decisions += typed.Count(a => a.Decision is not null);
            coarse += typed.Count(a => a.Coarse is not null);
        }

        evidence.Should().BeGreaterThan(0);
        decisions.Should().BeGreaterThan(0);
        coarse.Should().BeGreaterThan(0, "the fixture's schema has a coarse level");
    }

    [Fact]
    public void Evidence_reads_as_a_key_or_a_record()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        var observed = CoreTests.Observed();
        var kinds = new HashSet<Type>();
        foreach (var line in CoreTests.RecordLines()[200..260])
        {
            var log = JsonNode.Parse(line)!;
            var values = log["values"]!.AsObject()
                .Where(p => observed.Contains(p.Key))
                .ToDictionary(p => p.Key, p => (string)p.Value!);
            foreach (var candidate in decider.Answer(schema, (string)log["id"]!, values).SelectMany(a => a.Candidates))
            {
                switch (candidate.Evidence)
                {
                    case KeyEvidence key:
                        observed.Should().Contain(key.Attribute);
                        key.ToString().Should().Be($"{key.Attribute}: {key.Value}");
                        kinds.Add(typeof(KeyEvidence));
                        break;
                    case RecordEvidence record:
                        record.Similarity.Should().BeInRange(0, 1);
                        kinds.Add(typeof(RecordEvidence));
                        break;
                }
            }
        }

        kinds.Should().BeEquivalentTo([typeof(KeyEvidence), typeof(RecordEvidence)]);
    }

    [Fact]
    public void An_open_record_hands_back_answers_and_takes_an_external_judgement()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        using var record = OpenRecord.Open(schema, "x", DateTimeOffset.UnixEpoch);

        var fresh = CoreTests.Observed().SelectMany(name => record.Observe(decider, name, "never-seen-" + name)).ToList();
        fresh.Should().NotBeEmpty();
        var asking = record.Answer(decider).FirstOrDefault(a => a.NeedsExternal is not null);
        asking.Should().NotBeNull("with no known value nothing backs an answer");
        asking!.NeedsExternal!.Select(v => v.Attribute).Should().BeSubsetOf(CoreTests.Observed());

        var offered = record.OfferExternal(decider, asking.Attribute, [new ExternalCandidate("judged", 0.9, true)]);
        offered.Attribute.Should().Be(asking.Attribute);
        offered.Source.Should().Be(Source.External);
        offered.Candidates[0].Should().Be(new Candidate("judged", 0.9, Source.External, true));
        offered.Answered.Should().BeTrue();

        var settled = record.Settle(decider, asking.Attribute, SettlementKind.Accept, DateTimeOffset.UnixEpoch, "judged");
        settled.Should().NotContain(a => a.Attribute == asking.Attribute, "a settled attribute is not open");
    }

    [Fact]
    public void An_open_record_s_answers_carry_decision_records_when_asked()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        var log = JsonNode.Parse(CoreTests.RecordLines()[200])!;
        using var record = OpenRecord.Open(schema, (string)log["id"]!, DateTimeOffset.UnixEpoch);
        foreach (var name in CoreTests.Observed())
        {
            record.Observe(decider, name, (string?)log["values"]![name]);
        }

        record.Answer(decider).Should().AllSatisfy(a => a.Decision.Should().BeNull());
        var with = record.Answer(decider, decisions: true);
        with.Should().NotBeEmpty().And.AllSatisfy(a =>
        {
            a.Decision.Should().NotBeNull();
            a.Decision!.Attribute.Should().Be(a.Attribute);
            a.Decision.Record.Should().Be((string)log["id"]!);
            a.Decision.Weight.Should().Be(1, "an answer counts as one lookup");
        });
        with.Select(a => a with { Decision = null }).Should().BeEquivalentTo(record.Answer(decider));
    }

    [Fact]
    public async Task AnswerAsync_asks_the_judge_only_where_an_answer_asks_for_one()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        using var record = Unknown(schema, decider);
        var asking = record.Answer(decider).Where(a => a.NeedsExternal is not null).Select(a => a.Attribute).ToList();
        asking.Should().NotBeEmpty();
        var judge = new Judge(_ => [new ExternalCandidate("judged", 0.9, true)]);

        var answers = await record.AnswerAsync(decider, judge, TestContext.Current.CancellationToken);

        judge.Asked.Should().BeEquivalentTo(asking);
        judge.Seen.Should().AllSatisfy(a => a.Role.Should().Be(AttributeRole.Decided));
        answers.Where(a => asking.Contains(a.Attribute)).Should().AllSatisfy(a =>
        {
            a.Source.Should().Be(Source.External);
            a.Candidates[0].Value.Should().Be("judged");
        });
        answers.Select(a => a.Attribute).Should().Equal(record.Answer(decider).Select(a => a.Attribute));
    }

    [Fact]
    public async Task An_abstaining_or_outdated_judgement_leaves_the_answer_as_the_record_gives_it()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        using var record = Unknown(schema, decider);
        var before = record.Answer(decider);

        var abstained = await record.AnswerAsync(decider, new Judge(_ => []), TestContext.Current.CancellationToken);
        abstained.Should().BeEquivalentTo(before);

        var observed = CoreTests.Observed().First();
        var changing = new Judge(_ =>
        {
            record.Observe(decider, observed, "changed-while-judging");
            return [new ExternalCandidate("stale", 0.9, true)];
        });
        var after = await record.AnswerAsync(decider, changing, TestContext.Current.CancellationToken);
        after.SelectMany(a => a.Candidates).Should().NotContain(c => c.Value == "stale",
            "the judgement read values the record no longer has");
    }

    [Fact]
    public async Task A_canceled_answer_throws_and_a_failing_judge_fails_it()
    {
        var (schema, decider) = CoreTests.Filled();
        using var _ = schema;
        using var __ = decider;
        using var record = Unknown(schema, decider);
        using var canceled = new CancellationTokenSource();
        await canceled.CancelAsync();

        await record.Invoking(r => r.AnswerAsync(decider, new Judge(_ => []), canceled.Token))
            .Should().ThrowAsync<OperationCanceledException>();
        await record.Invoking(r => r.AnswerAsync(decider, new Judge(_ => throw new InvalidOperationException("down")),
                TestContext.Current.CancellationToken))
            .Should().ThrowAsync<InvalidOperationException>().WithMessage("down");
    }

    /// <summary>A record whose observed values no settled record has, so that nothing backs an answer.</summary>
    private static OpenRecord Unknown(Schema schema, Decider decider)
    {
        var record = OpenRecord.Open(schema, "x", DateTimeOffset.UnixEpoch);
        foreach (var name in CoreTests.Observed())
        {
            record.Observe(decider, name, "never-seen-" + name);
        }

        return record;
    }

    private sealed class Judge(Func<Attribute, IReadOnlyList<ExternalCandidate>> rank) : IExternalJudge
    {
        private readonly Lock _gate = new();

        public List<string> Asked { get; } = [];

        public List<Attribute> Seen { get; } = [];

        public ValueTask<IReadOnlyList<ExternalCandidate>> JudgeAsync(
            Attribute attribute, IReadOnlyList<ObservedValue> evidence, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            lock (_gate)
            {
                Asked.Add(attribute.Name);
                Seen.Add(attribute);
            }

            return ValueTask.FromResult(rank(attribute));
        }
    }

    /// <summary>The core writes nulls the binding leaves out, and the two spell a whole double differently (`160.0`, `160`).</summary>
    private static JsonNode? Comparable(JsonNode? node) => CoreTests.Doubles(node switch
    {
        JsonArray a => new JsonArray([.. a.Select(Comparable)]),
        JsonObject o => new JsonObject(o.Where(p => p.Value is not null).Select(p => KeyValuePair.Create(p.Key, Comparable(p.Value)))),
        _ => node?.DeepClone(),
    });
}
