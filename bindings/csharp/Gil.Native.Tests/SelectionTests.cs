using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.Json.Serialization.Metadata;
using AwesomeAssertions;

namespace Gil.Native.Tests;

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
        JsonNode.DeepEquals(Doubles(JsonNode.Parse(actual)), Doubles(expected)).Should().BeTrue(what);

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
        keyFields.Typed!.Thresholds().Should().HaveCount(keyFields.Typed.ByLength.Count);
    }

    private static JsonNode? Write<T>(T value, JsonTypeInfo<T> type) => JsonNode.Parse(JsonSerializer.Serialize(value, type));

    /// <summary>The C ABI writes nulls the binding leaves out.</summary>
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

        var captured = Tuning.CaptureJson(schema, (int)call["settled"]!, (double)call["target_precision"]!);
        Same(captured, call["captured"], "captured");
        var promised = Tuning.PromiseJson(captured, (string)call["attribute"]!, thresholds);
        Same(promised, call["promised"], "promised");
        using (var applied = Tuning.Apply(promised, schema))
        {
            Same(applied.ToJson(), call["applied"], "applied");
        }

        Tuning.StaleJson(promised, schema, Chosen).Should().BeNull();
        var stale = Tuning.StaleJson(promised, schema, (int)call["stale_at"]!);
        stale.Should().NotBeNull();
        Same(stale!, call["stale"], "stale");
        FluentActions.Invoking(() => Tuning.PromiseJson("{", "team", thresholds))
            .Should().Throw<ArgumentException>().WithMessage("a tuning does not read*");
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
        Usage.CountJson([]).Should().NotBeNullOrEmpty();
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
