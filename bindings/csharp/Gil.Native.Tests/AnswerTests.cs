using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Native.Tests;

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

    /// <summary>The core writes nulls the binding leaves out, and whole doubles without a fraction.</summary>
    private static JsonNode? Comparable(JsonNode? node) => CoreTests.Doubles(node switch
    {
        JsonArray a => new JsonArray([.. a.Select(Comparable)]),
        JsonObject o => new JsonObject(o.Where(p => p.Value is not null).Select(p => KeyValuePair.Create(p.Key, Comparable(p.Value)))),
        _ => node?.DeepClone(),
    });
}
