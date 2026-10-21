using System.Diagnostics;
using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Native.Tests;

/// <summary>The binding over the decision core's C ABI: the schema path's decision golden reproduced from .NET; open
/// records whose flat answers agree with the JSON ones, saved and reopened; refusals as exceptions; and how long the
/// answer asked for while a person types takes through the boundary.</summary>
public sealed class CoreTests(ITestOutputHelper output)
{
    private const int Chosen = 200;

    private static string Fixture(string name) => File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "core", name));

    private static string[] RecordLines() =>
        Fixture("records.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries);

    private static readonly Lazy<JsonObject> SchemaJson = new(() => JsonNode.Parse(Fixture("schema.json"))!.AsObject());

    private static IEnumerable<JsonObject> Attributes(string role) =>
        SchemaJson.Value["attributes"]!.AsArray().Select(a => a!.AsObject()).Where(a => (string)a["role"]! == role);

    private static HashSet<string> Observed() => [.. Attributes("observed").Select(a => (string)a["name"]!)];

    private static JsonArray Candidate(JsonNode c) =>
        [(string)c["value"]!, (double)c["score"]!, (string)c["source"]!, (bool)c["trusted"]!];

    [Fact]
    public void The_binding_reproduces_the_golden_decisions()
    {
        var lines = RecordLines();
        var golden = Fixture("golden.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(l => JsonNode.Parse(l)!)
            .Where(l => (string)l["kind"]! == "decision")
            .ToArray();
        var observed = Observed();

        using var schema = Schema.FromJson(Fixture("schema.json"));
        using var decider = new Decider();
        using (var early = SettledRecords.FromJsonLines(string.Join('\n', lines[..Chosen])))
        {
            early.Count.Should().Be(Chosen);
            decider.Put(schema, early);
        }

        var at = 0;
        foreach (var line in lines[Chosen..])
        {
            var record = JsonNode.Parse(line)!;
            var id = (string)record["id"]!;
            var values = record["values"]!.AsObject()
                .Where(p => observed.Contains(p.Key))
                .ToDictionary(p => p.Key, p => (string)p.Value!);
            foreach (var answer in JsonNode.Parse(decider.AnswerJson(schema, id, values))!.AsArray())
            {
                var actual = new JsonObject
                {
                    ["kind"] = "decision",
                    ["doc"] = id,
                    ["field"] = (string)answer!["attribute"]!,
                    ["answered"] = (bool)answer["answered"]!,
                    ["source"] = (string)answer["source"]!,
                    ["candidates"] = new JsonArray([.. answer["candidates"]!.AsArray().Select(c => (JsonNode)Candidate(c!))]),
                    ["coarse"] = answer["coarse"] is { } coarse ? Candidate(coarse) : null,
                };
                JsonNode.DeepEquals(Doubles(actual), Doubles(golden[at])).Should()
                    .BeTrue($"decision line {at + 1}: {actual.ToJsonString()} vs {golden[at].ToJsonString()}");
                at++;
            }

            using var settled = SettledRecords.FromJsonLines(line);
            decider.Put(schema, settled);
        }

        at.Should().Be(golden.Length);
    }

    /// <summary>Every number as the double it denotes: the reference writes a whole double without a fraction.</summary>
    private static JsonNode? Doubles(JsonNode? node) => node switch
    {
        JsonValue v when v.TryGetValue<double>(out var d) => JsonValue.Create(d),
        JsonArray a => new JsonArray([.. a.Select(Doubles)]),
        JsonObject o => new JsonObject(o.Select(p => KeyValuePair.Create(p.Key, Doubles(p.Value)))),
        _ => node?.DeepClone(),
    };

    private static (Schema Schema, Decider Decider) Filled()
    {
        var schema = Schema.FromJson(Fixture("schema.json"));
        var decider = new Decider();
        using var early = SettledRecords.FromJsonLines(string.Join('\n', RecordLines()[..Chosen]));
        decider.Put(schema, early);
        return (schema, decider);
    }

    [Fact]
    public void An_open_record_answers_flat_as_it_answers_in_JSON_and_reopens_as_it_was_left()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        var decided = Attributes("decided").Where(a => !(bool)a["multiple"]!).Select(a => (string)a["name"]!).ToArray();
        var observed = Observed();
        var compared = 0;

        foreach (var line in RecordLines()[Chosen..(Chosen + 30)])
        {
            var log = JsonNode.Parse(line)!;
            var values = log["values"]!.AsObject();
            using var record = OpenRecord.Open(schema, (string)log["id"]!, DateTimeOffset.UnixEpoch);
            foreach (var name in observed)
            {
                record.Observe(decider, name, (string?)values[name]);
            }

            var json = JsonNode.Parse(record.AnswerJson(decider))!.AsArray();
            foreach (var answer in json)
            {
                var name = (string)answer!["attribute"]!;
                if (!decided.Contains(name))
                {
                    continue;
                }

                var flat = record.AnswerPartial(decider, name);
                flat.Answered.Should().Be((bool)answer["answered"]!);
                flat.NeedsExternal.Should().Be(answer["needs_external"] is not null);
                flat.Layer.ToString().Should().Be((string)answer["source"]! == "Model" ? "Model" : (string)answer["source"]!);
                flat.Candidates.Select(c => (c.Value, c.Score, c.Trusted)).Should().Equal(
                    answer["candidates"]!.AsArray().Select(c => ((string)c!["value"]!, (double)c["score"]!, (bool)c["trusted"]!)));
                flat.Coarse?.Value.Should().Be((string?)answer["coarse"]?["value"]);
                compared++;
            }

            foreach (var name in decided)
            {
                if ((string?)values[name] is { } truth)
                {
                    record.Settle(decider, name, SettlementKind.Correct, DateTimeOffset.UnixEpoch.AddDays(1), truth);
                }
            }

            var saved = record.SnapshotJson();
            using var reopened = OpenRecord.Reopen(schema, saved);
            reopened.SnapshotJson().Should().Be(saved);
            reopened.AnswerJson(decider).Should().Be(record.AnswerJson(decider));
        }

        compared.Should().BeGreaterThan(20);
    }

    [Fact]
    public void A_refused_call_is_an_argument_exception_with_the_core_s_message()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        using var record = OpenRecord.Open(schema, "r", DateTimeOffset.UnixEpoch);
        var decided = (string)Attributes("decided").First(a => !(bool)a["multiple"]!)["name"]!;

        FluentActions.Invoking(() => Schema.FromJson("{\"name\":"))
            .Should().Throw<ArgumentException>().WithMessage("a schema does not read*");
        FluentActions.Invoking(() => record.Settle(decider, decided, SettlementKind.Accept, DateTimeOffset.UnixEpoch))
            .Should().Throw<ArgumentException>().WithMessage("a Accept settlement needs a value");
        FluentActions.Invoking(() => record.Observe(decider, decided, "x"))
            .Should().Throw<ArgumentException>().WithMessage($"'{decided}' is decided; settle it instead");
        FluentActions.Invoking(() => OpenRecord.Open(schema, " ", DateTimeOffset.UnixEpoch))
            .Should().Throw<ArgumentException>().WithMessage("a record needs an id");

        FluentActions.Invoking(() => record.Observe(decider, "nowhere", "x"))
            .Should().Throw<ArgumentException>().WithMessage("*has no attribute 'nowhere'");

        // The binding goes on working after a refusal: a changing call refuses before it changes anything.
        record.AnswerPartial(decider, decided).Candidates.Should().NotBeEmpty();
        record.Observe(decider, Observed().First(), "x");
    }

    [Fact]
    public void Answers_carry_decision_records_when_asked_and_a_removed_record_is_never_offered_again()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        var observed = Observed();
        var early = RecordLines()[..Chosen].Select(l => (string)JsonNode.Parse(l)!["id"]!).ToArray();
        var later = RecordLines()[Chosen..].Take(60).Select(l => JsonNode.Parse(l)!).ToArray();
        Dictionary<string, string> Values(JsonNode log) => log["values"]!.AsObject()
            .Where(p => observed.Contains(p.Key))
            .ToDictionary(p => p.Key, p => (string)p.Value!);
        IEnumerable<string> Cited() => later
            .SelectMany(log => JsonNode.Parse(decider.AnswerJson(schema, (string)log["id"]!, Values(log)))!.AsArray())
            .SelectMany(a => a!["candidates"]!.AsArray())
            .Select(c => c!["evidence"]?["record"]?["id"])
            .Where(id => id is not null)
            .Select(id => (string)id!);

        var first = later[0];
        var plain = JsonNode.Parse(decider.AnswerJson(schema, (string)first["id"]!, Values(first)))!.AsArray();
        var recorded = JsonNode.Parse(
            decider.AnswerJson(schema, (string)first["id"]!, Values(first), decisions: true))!.AsArray();
        plain.Should().AllSatisfy(a => a!["decision"].Should().BeNull());
        recorded.Should().HaveCount(plain.Count);
        foreach (var (with, without) in recorded.Zip(plain))
        {
            var decision = with!["decision"]!;
            ((string)decision["attribute"]!).Should().Be((string)without!["attribute"]!);
            ((string)decision["record"]!).Should().Be((string)first["id"]!);
            with.AsObject().Remove("decision");
            JsonNode.DeepEquals(with, without).Should().BeTrue();
        }

        var cited = Cited().ToHashSet();
        cited.Should().NotBeEmpty("later records lean on similar early ones");
        foreach (var id in cited)
        {
            decider.Remove(schema, id);
        }

        decider.Remove(schema, "never-put");
        Cited().Should().NotIntersectWith(cited);
        early.Should().Contain(cited);
    }

    [Fact]
    public void Answers_from_one_decider_run_in_parallel_and_agree_with_serial_ones()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        var observed = Observed();
        var logs = RecordLines()[Chosen..].Take(64).Select(l => JsonNode.Parse(l)!).ToArray();
        string Answer(JsonNode log)
        {
            var values = log["values"]!.AsObject()
                .Where(p => observed.Contains(p.Key))
                .ToDictionary(p => p.Key, p => (string)p.Value!);
            return decider.AnswerJson(schema, (string)log["id"]!, values);
        }

        var serial = logs.Select(Answer).ToArray();
        var parallel = new string[logs.Length];
        Parallel.For(0, logs.Length, new ParallelOptions { MaxDegreeOfParallelism = 8 }, i => parallel[i] = Answer(logs[i]));
        parallel.Should().Equal(serial);
    }

    [Fact]
    public void A_set_attribute_settles_with_values_and_an_empty_set_clears_it()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        var set = (string?)Attributes("decided").FirstOrDefault(a => (bool)a["multiple"]!)?["name"];
        if (set is null)
        {
            return;
        }

        using var record = OpenRecord.Open(schema, "s", DateTimeOffset.UnixEpoch);
        record.Settle(decider, set, SettlementKind.Set, DateTimeOffset.UnixEpoch, values: ["b", "a", "a"]);
        JsonNode.Parse(record.SnapshotJson())!["sets"]![set]!.AsArray().Select(v => (string)v!)
            .Should().Equal("a", "b");
        record.Settle(decider, set, SettlementKind.Set, DateTimeOffset.UnixEpoch, values: []);
        JsonNode.Parse(record.SnapshotJson())!["sets"]![set].Should().BeNull();
    }

    [Fact]
    public void The_answer_while_typing_is_quick_through_the_boundary()
    {
        var (schema, decider) = Filled();
        using var _ = schema;
        using var __ = decider;
        var decided = Attributes("decided").Where(a => !(bool)a["multiple"]!).Select(a => (string)a["name"]!).ToArray();
        var log = JsonNode.Parse(RecordLines()[Chosen])!;
        using var record = OpenRecord.Open(schema, "typing", DateTimeOffset.UnixEpoch);
        foreach (var name in Observed())
        {
            record.Observe(decider, name, (string?)log["values"]![name]);
        }

        string[] typed = [null!, "a", "ab", "p", "s", "lo"];
        for (var warm = 0; warm < 200; warm++)
        {
            record.AnswerPartial(decider, decided[warm % decided.Length], typed[warm % typed.Length]);
        }

        var times = new double[2_000];
        for (var i = 0; i < times.Length; i++)
        {
            var started = Stopwatch.GetTimestamp();
            record.AnswerPartial(decider, decided[i % decided.Length], typed[i % typed.Length]);
            times[i] = Stopwatch.GetElapsedTime(started).TotalMicroseconds;
        }

        Array.Sort(times);
        var p50 = times[times.Length / 2];
        var p95 = times[times.Length * 95 / 100];
        output.WriteLine($"answer while typing, core work included: p50 {p50:F1} µs, p95 {p95:F1} µs");
        p95.Should().BeLessThan(5_000, "a debug build on a shared machine still answers well inside a keystroke");
    }
}
