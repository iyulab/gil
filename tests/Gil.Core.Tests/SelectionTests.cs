using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>The selections, the tuning round trip and the usage counts from .NET give what the C ABI gave for the same
/// inputs (<c>core/selections.json</c>, written by the C ABI's own tests, which hold it to the core).</summary>
public sealed class SelectionTests
{
    private const int Chosen = 200;

    private static string Fixture(string name) => File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "core", name));

    private static readonly Lazy<JsonArray> Calls = new(() => JsonNode.Parse(Fixture("selections.json"))!.AsArray());

    private static IEnumerable<JsonObject> Of(string call) =>
        Calls.Value.Select(c => c!.AsObject()).Where(c => (string)c["call"]! == call);

    private static (Schema Schema, SettledRecords Records) Early()
    {
        var lines = Fixture("records.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries);
        return (Schema.FromJson(Fixture("schema.json")), SettledRecords.FromJsonLines(string.Join('\n', lines[..Chosen])));
    }

    private static SelectionOptions Options(JsonNode o) => new((double)o["target_precision"]!, (int)o["minimum_answered"]!)
    {
        Counting = o["counting"]?.AsArray().Select(k => Enum.Parse<SettlementKind>((string)k!)).ToArray(),
        Rule = (string?)o["rule"] == "pooled" ? ThresholdRule.Pooled : ThresholdRule.EveryBand,
        Decisions = (bool?)o["decisions"] ?? false,
        TypedLongest = (int?)o["typed_longest"] ?? 0,
    };

    private static void Same(string actual, JsonNode? expected, string what) =>
        JsonNode.DeepEquals(Comparable(JsonNode.Parse(actual)), Comparable(expected)).Should().BeTrue(what);

    /// <summary>Every number as the double it denotes.</summary>
    private static JsonNode? Doubles(JsonNode? node) => node switch
    {
        JsonValue v when v.TryGetValue<double>(out var d) => JsonValue.Create(d),
        JsonArray a => new JsonArray([.. a.Select(Doubles)]),
        JsonObject o => new JsonObject(o.Select(p => KeyValuePair.Create(p.Key, Doubles(p.Value)))),
        _ => node?.DeepClone(),
    };

    [Fact]
    public void The_selections_give_what_the_C_ABI_gave()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        var seen = 0;
        foreach (var call in Calls.Value.Select(c => c!.AsObject()))
        {
            var attribute = (string?)call["attribute"];
            var found = (string)call["call"]! switch
            {
                "layers" => Replay.SelectLayersJson(schema, attribute!, records, Options(call["options"]!)),
                "coarse" => Replay.SelectCoarseJson(schema, attribute!, records, Options(call["options"]!)),
                "key_fields" => Replay.SelectKeyFieldsJson(schema, attribute!, records, Options(call["options"]!)),
                "key_concentrations" => Replay.SelectKeyConcentrationsJson(schema, attribute!, records),
                _ => null,
            };
            if (found is null)
            {
                continue;
            }

            Same(found, call["result"], $"{call["call"]} {attribute}");
            seen++;
        }

        seen.Should().Be(5);
    }

    [Fact]
    public void Typed_selections_hold_everything_the_C_ABI_gave()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        var seen = 0;
        foreach (var call in Calls.Value.Select(c => c!.AsObject()))
        {
            var attribute = (string?)call["attribute"];
            JsonNode? written = (string)call["call"]! switch
            {
                "layers" => Write(Replay.SelectLayers(schema, attribute!, records, Options(call["options"]!)), CoreJson.Default.LayerSelection),
                "coarse" => Write(Replay.SelectCoarse(schema, attribute!, records, Options(call["options"]!)), CoreJson.Default.LayerSelection),
                "key_fields" => Write(Replay.SelectKeyFields(schema, attribute!, records, Options(call["options"]!)), CoreJson.Default.KeyFieldsChoice),
                "key_concentrations" => Write(Replay.SelectKeyConcentrations(schema, attribute!, records), CoreJson.Default.IReadOnlyDictionaryStringDouble),
                _ => null,
            };
            if (written is null)
            {
                continue;
            }

            JsonNode.DeepEquals(Comparable(written), Comparable(call["result"])).Should()
                .BeTrue($"{call["call"]} {attribute}: {written.ToJsonString()}");
            seen++;
        }

        seen.Should().Be(5);
        var pooled = Replay.SelectLayers(schema, "code", records, new(0.7, 5) { Rule = ThresholdRule.Pooled, Decisions = true });
        pooled.Decisions.Should().NotBeEmpty().And.AllSatisfy(d => d.Attribute.Should().Be("code"));
        var keyFields = Replay.SelectKeyFields(schema, "team", records, new(0.8, 10) { TypedLongest = 3 });
        keyFields.Typed!.Thresholds.Should().HaveCount(keyFields.Typed.ByLength.Count);
    }

    private static JsonNode? Write<T>(T value, JsonTypeInfo<T> type) => JsonNode.Parse(JsonSerializer.Serialize(value, type));

    [Fact]
    public void Records_that_name_their_batch_get_a_batch_check_and_the_same_thresholds()
    {
        var lines = Fixture("records.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries)[..Chosen];
        var batched = lines.Select((line, i) =>
        {
            var record = JsonNode.Parse(line)!.AsObject();
            record["batch"] = $"b{i / 4}";
            return record.ToJsonString();
        });
        using var schema = Schema.FromJson(Fixture("schema.json"));
        using var plainRecords = SettledRecords.FromJsonLines(string.Join('\n', lines));
        using var batchedRecords = SettledRecords.FromJsonLines(string.Join('\n', batched));
        var options = new SelectionOptions(0.8, 10);

        var plain = Replay.SelectLayers(schema, "team", plainRecords, options).Thresholds;
        var named = Replay.SelectLayers(schema, "team", batchedRecords, options).Thresholds;

        plain.Batches.Should().BeNull();
        named.Batches.Should().NotBeNull();
        named.Batches!.Batches.Should().Be((Chosen + 3) / 4);
        named.Batches.AcrossBatchesAnswered.Should().BePositive();
        named.Batches.AcrossRecordsAnswered.Should().BePositive();
        // The check moves no threshold.
        named.Key.Should().BeEquivalentTo(plain.Key);
        named.Memory.Should().BeEquivalentTo(plain.Memory);
        // A record's batch crosses to the core and back; one that names none writes none.
        var record = new Record("r1", DateTimeOffset.UnixEpoch) { Batch = "import-7" };
        JsonNode.Parse(JsonSerializer.Serialize(record, CoreJson.Default.Record))!["batch"]!.GetValue<string>().Should().Be("import-7");
        JsonNode.Parse(JsonSerializer.Serialize(record with { Batch = null }, CoreJson.Default.Record))!.AsObject().ContainsKey("batch").Should().BeFalse();
    }

    /// <summary>The C ABI writes nulls the binding leaves out — the core reads both as nothing.</summary>
    private static JsonNode? Comparable(JsonNode? node) => Doubles(node switch
    {
        JsonArray a => new JsonArray([.. a.Select(Comparable)]),
        JsonObject o => new JsonObject(o.Where(p => p.Value is not null).Select(p => KeyValuePair.Create(p.Key, Comparable(p.Value)))),
        _ => node?.DeepClone(),
    });

    [Fact]
    public void A_tuning_round_trip_gives_what_the_C_ABI_gave()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        var call = Of("tuning").Single();
        var thresholds = Of("layers").First()["result"]!["thresholds"]!.ToJsonString();

        var captured = Tuning.Capture(schema, (int)call["settled"]!, (double)call["target_precision"]!);
        Same(captured.ToJson(), call["captured"], "captured");
        Tuning.FromJson(captured.ToJson()).Should().BeEquivalentTo(captured);
        var layers = Replay.SelectLayers(schema, (string)call["attribute"]!, records, Options(Of("layers").First()["options"]!));
        Same(JsonSerializer.Serialize(layers.Thresholds, CoreJson.Default.LayerThresholds), JsonNode.Parse(thresholds), "thresholds");
        var promised = captured.Promise((string)call["attribute"]!, layers.Thresholds);
        Same(promised.ToJson(), call["promised"], "promised");
        promised.Attributes.Single(a => a.Attribute == (string)call["attribute"]!).KeyPromise.Should().NotBeNull();
        using (var applied = promised.Apply(schema))
        {
            Same(applied.ToJson(), call["applied"], "applied");
        }

        promised.Stale(schema, Chosen).Should().BeNull();
        var stale = promised.Stale(schema, (int)call["stale_at"]!);
        stale.Should().BeOfType<Stale.Settled>().Which.Now.Should().Be((int)call["stale_at"]!);
        Same(JsonSerializer.Serialize(stale, CoreJson.Default.Stale), call["stale"], "stale");
        (promised with { Version = "0.0.1" }).Stale(schema, Chosen).Should().Be(new Stale.Version("0.0.1"));
        FluentActions.Invoking(() => Tuning.FromJson("{"))
            .Should().Throw<ArgumentException>().WithMessage("a tuning does not read*");
        FluentActions.Invoking(() => captured.Promise("nowhere", layers.Thresholds))
            .Should().Throw<ArgumentException>();
    }

    [Fact]
    public void The_usage_counts_give_what_the_C_ABI_gave()
    {
        var call = Of("usage").Single();
        var decisions = call["decisions"]!.AsArray().Select(d => new UsageDecision(
            (string)d!["attribute"]!,
            (string?)d["answer"],
            (string?)d["settled"],
            d["settlement"] is { } kind ? Enum.Parse<SettlementKind>((string)kind!) : null,
            (double)d["probability"]!,
            (double?)d["promise"])).ToArray();
        Same(Usage.CountJson(decisions), call["result"], "usage");
        var counted = Usage.Count(decisions);
        Same(JsonSerializer.Serialize(counted, CoreJson.Default.IReadOnlyDictionaryStringAttributeUsage), call["result"], "typed usage");
        var team = counted["team"];
        team.Abstained.Should().Be(team.Decisions - team.Answered);
        team.Realized.Should().Be(team.RightWeight / team.JudgedWeight);
        Usage.Count([]).Should().BeEmpty();
    }

    [Fact]
    public void A_selection_the_contract_rules_out_is_an_argument_exception()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        FluentActions.Invoking(() => Replay.SelectLayersJson(schema, "product", records, new(0.8, 10)))
            .Should().Throw<ArgumentException>().WithMessage("'product' is not a decided attribute");
        FluentActions.Invoking(() => Replay.SelectLayersJson(schema, "team", records, new(1.5, 10)))
            .Should().Throw<ArgumentException>().WithMessage("target_precision must be in (0, 1]");
    }
}
