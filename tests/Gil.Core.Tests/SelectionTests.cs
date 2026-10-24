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

    /// <summary>A fixture's tuning with the core's version in place of the placeholder the C ABI's tests write there (the
    /// fixture does not change with every release; those tests hold the version to the crate's).</summary>
    private static JsonNode? Versioned(JsonNode? tuning, string version)
    {
        var copy = tuning!.DeepClone();
        ((string)copy["version"]!).Should().Be("{version}");
        version.Should().NotBeNullOrEmpty();
        copy["version"] = version;
        return copy;
    }

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

    /// <summary>What was in view, how long it took and who settled it cross to the core under the names the line format
    /// gives them; one it cannot take — a promise out of [0, 1] — is refused as input.</summary>
    [Fact]
    public void RecordCarriesWhatWasInViewHowLongItTookAndWhoSettledIt()
    {
        var record = new Record("r1", DateTimeOffset.UnixEpoch)
        {
            Values = new Dictionary<string, string> { ["team"] = "network" },
            Settlements = new Dictionary<string, SettlementKind> { ["team"] = SettlementKind.Correct },
            Suggested = new Dictionary<string, string> { ["team"] = "desk" },
            Sources = new Dictionary<string, Source> { ["team"] = Source.External },
            Promises = new Dictionary<string, double> { ["team"] = 0.9 },
            SettleMs = new Dictionary<string, ulong> { ["team"] = 4200 },
            OpenMs = 61000,
            SettledBy = new Dictionary<string, string> { ["team"] = "p-17" },
            FormVersion = "9f2c",
        };
        var line = JsonNode.Parse(JsonSerializer.Serialize(record, CoreJson.Default.Record))!.AsObject();
        line["suggested"]!["team"]!.GetValue<string>().Should().Be("desk");
        line["sources"]!["team"]!.GetValue<string>().Should().Be("Model");
        line["promises"]!["team"]!.GetValue<double>().Should().Be(0.9);
        line["settle_ms"]!["team"]!.GetValue<ulong>().Should().Be(4200);
        line["open_ms"]!.GetValue<ulong>().Should().Be(61000);
        line["settled_by"]!["team"]!.GetValue<string>().Should().Be("p-17");
        line["form_version"]!.GetValue<string>().Should().Be("9f2c");
        using (SettledRecords.From([record])) { }

        var bare = JsonNode.Parse(JsonSerializer.Serialize(new Record("r2", DateTimeOffset.UnixEpoch), CoreJson.Default.Record))!.AsObject();
        bare.Select(p => p.Key).Should().NotIntersectWith(["suggested", "sources", "promises", "settle_ms", "open_ms", "settled_by", "form_version"]);

        FluentActions.Invoking(() => SettledRecords.From([record with { Promises = new Dictionary<string, double> { ["team"] = 1.2 } }]))
            .Should().Throw<ArgumentException>().WithMessage("*not in [0, 1]*");
    }

    /// <summary>The C ABI writes nulls the binding leaves out — the core reads both as nothing.</summary>
    private static JsonNode? Comparable(JsonNode? node) => Doubles(node switch
    {
        JsonArray a => new JsonArray([.. a.Select(Comparable)]),
        JsonObject o => new JsonObject(o.Where(p => p.Value is not null).Select(p => KeyValuePair.Create(p.Key, Comparable(p.Value)))),
        _ => node?.DeepClone(),
    });

    [Fact]
    public void A_replay_or_a_decider_kept_on_one_thread_gives_the_same()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        var call = Of("layers").First();
        var options = Options(call["options"]!);
        var single = Replay.SelectLayersJson(schema, (string)call["attribute"]!, records, options with { Threads = 1 });
        Same(single, call["result"], "one thread");

        using var decider = new Decider(threads: 1);
        decider.Put(schema, records);
        FluentActions.Invoking(() => new Decider(threads: 0)).Should().Throw<ArgumentOutOfRangeException>();
        FluentActions.Invoking(() => Replay.SelectLayersJson(schema, (string)call["attribute"]!, records, options with { Threads = 0 }))
            .Should().Throw<ArgumentOutOfRangeException>();
    }

    [Fact]
    public void A_tuning_round_trip_gives_what_the_C_ABI_gave()
    {
        var (schema, records) = Early();
        using var _ = schema;
        using var __ = records;
        var call = Of("tuning").Single();
        var thresholds = Of("layers").First()["result"]!["thresholds"]!.ToJsonString();

        var captured = Tuning.Capture(schema, (int)call["settled"]!, (double)call["target_precision"]!);
        Same(captured.ToJson(), Versioned(call["captured"], captured.Version), "captured");
        Tuning.FromJson(captured.ToJson()).Should().BeEquivalentTo(captured);
        var layers = Replay.SelectLayers(schema, (string)call["attribute"]!, records, Options(Of("layers").First()["options"]!));
        Same(JsonSerializer.Serialize(layers.Thresholds, CoreJson.Default.LayerThresholds), JsonNode.Parse(thresholds), "thresholds");
        var promised = captured.Promise((string)call["attribute"]!, layers.Thresholds);
        Same(promised.ToJson(), Versioned(call["promised"], promised.Version), "promised");
        promised.Attributes.Single(a => a.Attribute == (string)call["attribute"]!).KeyPromise.Should().NotBeNull();
        using (var applied = promised.Apply(schema))
        {
            Same(applied.ToJson(), call["applied"], "applied");
        }

        using (var withLayers = schema.WithLayers((string)call["attribute"]!, layers.Thresholds))
        {
            Same(withLayers.ToJson(), call["with_layers"], "with layers");
        }

        FluentActions.Invoking(() => schema.WithCoarse((string)call["attribute"]!, layers.Thresholds))
            .Should().Throw<ArgumentException>().WithMessage("*no coarse level*");
        var attribute = (string)call["attribute"]!;
        var tuned = promised.Attributes.Single(a => a.Attribute == attribute);
        promised.PromiseOf(attribute, Source.KeyMemory).Should().Be(tuned.KeyPromise);
        promised.PromiseOf(attribute, Source.SimilarRecord).Should().Be(tuned.MemoryPromise);
        promised.PromiseOf(attribute, Source.None).Should().BeNull();
        promised.PromiseOf(attribute, Source.External).Should().BeNull();
        promised.PromiseOf("nowhere", Source.KeyMemory).Should().BeNull();

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
        using var records = SettledRecords.FromJsonLines(
            string.Join("\n", call["records"]!.AsArray().Select(r => r!.ToJsonString())));
        Same(Usage.CountJson(records), call["result"], "usage");
        var counted = Usage.Count(records);
        Same(JsonSerializer.Serialize(counted, CoreJson.Default.IReadOnlyDictionaryStringAttributeUsage), call["result"], "typed usage");
        var team = counted["team"];
        team.Abstained.Should().Be(team.Decisions - team.Answered);
        team.Realized.Should().Be(team.RightWeight / team.JudgedWeight);
        using var none = SettledRecords.From([]);
        Usage.Count(none).Should().BeEmpty();
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

    [Fact]
    public void Records_read_their_time_as_text_as_they_read_it_in_ticks()
    {
        var lines = Fixture("records.jsonl").Split('\n', StringSplitOptions.RemoveEmptyEntries)[..Chosen];
        var asText = lines.Select(line =>
        {
            var record = JsonNode.Parse(line)!.AsObject();
            var ticks = (long)record["settled_at"]!;
            record["settled_at"] = new DateTimeOffset(DateTimeOffset.UnixEpoch.UtcTicks + ticks, TimeSpan.FromHours(9))
                .ToOffset(TimeSpan.FromHours(9)).ToString("O");
            record["probabilities"] ??= null;
            return record.ToJsonString();
        });
        using var schema = Schema.FromJson(Fixture("schema.json"));
        using var inTicks = SettledRecords.FromJsonLines(string.Join('\n', lines));
        using var inText = SettledRecords.FromJsonLines(string.Join('\n', asText));

        var options = new SelectionOptions(0.8, 10);
        Replay.SelectLayersJson(schema, "team", inText, options)
            .Should().Be(Replay.SelectLayersJson(schema, "team", inTicks, options));
        FluentActions.Invoking(() => SettledRecords.FromJsonLines("""{"id":"t","settled_at":"2025-02-29T00:00:00.0000000+00:00"}"""))
            .Should().Throw<ArgumentException>().WithMessage("*neither ticks nor a time*");
    }
}
