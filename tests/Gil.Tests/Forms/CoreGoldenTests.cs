using System.Collections;
using System.Reflection;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>
/// The decision core's golden: synthetic settled records in, the chosen settings and every decision out, as JSON lines
/// another implementation can reproduce byte for byte (tests/fixtures/core). The records hold text the normalisation
/// specification turns on (docs/text-normalization.md) — Hangul, Greek, Turkish, full-width forms, characters beyond
/// U+FFFF. Set GIL_WRITE_CORE_GOLDEN to rewrite the files from this implementation.
/// </summary>
public sealed partial class CoreGoldenTests
{
    private const int Chosen = 200; // settings are chosen on the first records, decisions made on the rest

    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly JsonSerializerOptions Json = new() { Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping };

    private static readonly SettlementKind[] Counted = [SettlementKind.Enter, SettlementKind.Accept, SettlementKind.Correct, SettlementKind.Check];

    private static FormDefinition Form() => new("ticket",
    [
        new FieldDefinition("channel", FieldRole.Observed),
        new FieldDefinition("product", FieldRole.Observed),
        new FieldDefinition("summary", FieldRole.Observed),
        new FieldDefinition("team", FieldRole.Judged),
        new FieldDefinition("code", FieldRole.Judged) { Coarse = new CoarseLevel(2) },
        new FieldDefinition("tags", FieldRole.Judged) { Multiple = true },
    ], PromptLanguage.English);

    [Fact]
    public async Task The_core_reproduces_its_golden()
    {
        var directory = Path.Combine(AppContext.BaseDirectory, "core");
        var records = Path.Combine(directory, "records.jsonl");
        var golden = Path.Combine(directory, "golden.jsonl");
        if (Environment.GetEnvironmentVariable("GIL_WRITE_CORE_GOLDEN") is { } target)
        {
            Directory.CreateDirectory(target);
            await File.WriteAllLinesAsync(Path.Combine(target, "records.jsonl"), Generate().Select(Write), Ct);
            var (written, keys, replays, layers, decisionRecords) = await RunAsync([.. (await File.ReadAllLinesAsync(Path.Combine(target, "records.jsonl"), Ct)).Select(Read)]);
            await File.WriteAllLinesAsync(Path.Combine(target, "golden.jsonl"), written, Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "keys.jsonl"), keys, Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "replay.jsonl"), replays, Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "layers.jsonl"), layers, Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "records-out.jsonl"), decisionRecords, Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "inspection.jsonl"), Inspections().Select(Write), Ct);
            await File.WriteAllLinesAsync(
                Path.Combine(target, "key_fields.jsonl"),
                KeyFieldLines(
                    [.. (await File.ReadAllLinesAsync(Path.Combine(target, "records.jsonl"), Ct)).Select(Read)],
                    [.. (await File.ReadAllLinesAsync(Path.Combine(target, "inspection.jsonl"), Ct)).Select(Read)]),
                Ct);
            await File.WriteAllLinesAsync(Path.Combine(target, "lexical.jsonl"), await LexicalAsync([.. (await File.ReadAllLinesAsync(Path.Combine(target, "records.jsonl"), Ct)).Select(Read)]), Ct);
            return;
        }

        var documents = (await File.ReadAllLinesAsync(records, Ct)).Select(Read).ToList();

        var (lines, keyLines, replayLines, layerLines, recordLines) = await RunAsync(documents);

        lines.Should().Equal(await File.ReadAllLinesAsync(golden, Ct));
        keyLines.Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "keys.jsonl"), Ct));
        replayLines.Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "replay.jsonl"), Ct));
        layerLines.Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "layers.jsonl"), Ct));
        recordLines.Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "records-out.jsonl"), Ct));
        var inspections = (await File.ReadAllLinesAsync(Path.Combine(directory, "inspection.jsonl"), Ct)).Select(Read).ToList();
        KeyFieldLines(documents, inspections).Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "key_fields.jsonl"), Ct));
        (await LexicalAsync(documents)).Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "lexical.jsonl"), Ct));
    }

    /// <summary>
    /// The settings chosen on the first records, then a suggestion for every judged field of every later record before it
    /// is put — and, apart, the key layer alone: what the settled field memory ranks for each single-valued judged field
    /// of every later record, with the same settings (keys.jsonl), so another implementation can match that layer first —
    /// and the key layer's threshold replay alone for each single-valued judged field, on the first records with the form
    /// as it stands when that field's layers are chosen (replay.jsonl) — and both layers' replay, with that form, on the first
    /// records followed by some of them settled again and some copied under new ids, so documents are passed over as their
    /// own evidence and cases of the same evidence keep only the latest (layers.jsonl) — with the decision record of every
    /// document that replay asks about (records-out.jsonl).
    /// </summary>
    private static async Task<(List<string> Golden, List<string> Keys, List<string> Replays, List<string> Layers, List<string> Records)> RunAsync(IReadOnlyList<SettledDocument> documents)
    {
        var form = Form();
        var early = documents.Take(Chosen).ToList();
        var lines = new List<string>();
        var replayLines = new List<string>();
        var layerLines = new List<string>();
        var recordLines = new List<string>();
        var again = early.Concat(Enumerable.Range(0, 40).Select(i => early[i] with
        {
            DocumentId = i % 2 == 0 ? early[i].DocumentId : $"{early[i].DocumentId}-copy",
            SettledAt = early[^1].SettledAt.AddMinutes(i + 1),
        })).ToList();
        async Task KeyThreshold(string field)
        {
            replayLines.Add(Line(new JsonObject
            {
                ["kind"] = "key_threshold",
                ["field"] = field,
                ["replay"] = Replay(ThresholdSelection.SelectKeyThreshold(new FieldMemory(), form, field, early, 0.8, 10, Counted)),
            }));
            var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, field, again, 0.8, 10, Counted, Ct);
            layerLines.Add(Line(new JsonObject { ["kind"] = "layers_again", ["field"] = field, ["key"] = Replay(layers.Key), ["memory"] = Replay(layers.Memory) }));
            recordLines.AddRange(await RecordLinesAsync(form, field, again));
        }

        var keyFields = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "team", early, 0.8, 10, counting: Counted);
        lines.Add(Line(new JsonObject { ["kind"] = "key_fields", ["field"] = "team", ["fields"] = Strings(keyFields.KeyFields), ["chosen"] = Replay(keyFields.Chosen) }));
        form = With(form, form.Field("team") with { KeyFields = keyFields.KeyFields });

        var concentrations = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), form, "team", early);
        lines.Add(Line(new JsonObject
        {
            ["kind"] = "key_concentrations",
            ["field"] = "team",
            ["concentrations"] = new JsonObject([.. concentrations.OrderBy(c => c.Key, StringComparer.Ordinal).Select(c => KeyValuePair.Create(c.Key, (JsonNode?)c.Value))]),
        }));
        form = With(form, form.Field("team") with { KeyConcentrations = concentrations });

        await KeyThreshold("team");
        var team = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "layers", ["field"] = "team", ["key"] = Replay(team.Key), ["memory"] = Replay(team.Memory) }));
        form = With(form, form.Field("team") with
        {
            KeyThreshold = team.Key.Chosen?.Threshold,
            MemoryThreshold = team.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = team.Memory.Chosen?.SimilarityFloor,
        });

        await KeyThreshold("code");
        var code = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "code", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "layers", ["field"] = "code", ["key"] = Replay(code.Key), ["memory"] = Replay(code.Memory) }));
        form = With(form, form.Field("code") with
        {
            KeyThreshold = code.Key.Chosen?.Threshold,
            MemoryThreshold = code.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = code.Memory.Chosen?.SimilarityFloor,
        });

        var coarse = await ThresholdSelection.SelectCoarseAsync(new FieldMemory(), new LexicalMemory(), form, "code", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "coarse", ["field"] = "code", ["key"] = Replay(coarse.Key), ["memory"] = Replay(coarse.Memory) }));
        form = With(form, form.Field("code") with
        {
            Coarse = form.Field("code").Coarse! with
            {
                KeyThreshold = coarse.Key.Chosen?.Threshold,
                MemoryThreshold = coarse.Memory.Chosen?.Threshold,
                MemorySimilarityFloor = coarse.Memory.Chosen?.SimilarityFloor,
            },
        });

        var tags = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), form, "tags", early, 0.8, 10); // every settlement counted: a set is settled as Set
        lines.Add(Line(new JsonObject { ["kind"] = "key", ["field"] = "tags", ["replay"] = Replay(tags) }));
        form = With(form, form.Field("tags") with { KeyThreshold = tags.Chosen?.Threshold });

        var keyLines = new List<string>();
        var keyMemory = new FieldMemory();
        foreach (var document in early)
        {
            keyMemory.Put(form, FormResolver.Remembered(document));
        }

        foreach (var document in documents.Skip(Chosen))
        {
            var observed = document.Values.Where(v => form.Field(v.Key).Role == FieldRole.Observed).ToDictionary();
            foreach (var field in form.Fields.Where(f => f.Role == FieldRole.Judged && !f.Multiple))
            {
                keyLines.Add(Line(new JsonObject
                {
                    ["kind"] = "key_rank",
                    ["doc"] = document.DocumentId,
                    ["field"] = field.Name,
                    ["candidates"] = new JsonArray([.. keyMemory.Rank(form, field.Name, observed, 3).Select(c => (JsonNode)new JsonArray(c.Value, c.Score, c.Evidence, c.Trusted))]),
                }));
            }

            keyMemory.Put(form, FormResolver.Remembered(document));
        }

        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        await resolver.RebuildAsync(form, early, Ct);
        foreach (var document in documents.Skip(Chosen))
        {
            var observed = document.Values.Where(v => form.Field(v.Key).Role == FieldRole.Observed).ToDictionary();
            foreach (var suggestion in await resolver.SuggestAsync(form, document.DocumentId, observed, Ct))
            {
                lines.Add(Line(new JsonObject
                {
                    ["kind"] = "decision",
                    ["doc"] = document.DocumentId,
                    ["field"] = suggestion.Field,
                    ["answered"] = suggestion.Answered,
                    ["source"] = suggestion.Source.ToString(),
                    ["candidates"] = new JsonArray([.. suggestion.Candidates.Take(3).Select(c => (JsonNode)new JsonArray(c.Value, c.Score, c.Source.ToString(), c.Trusted))]),
                    ["coarse"] = suggestion.Coarse is { } k ? new JsonArray(k.Value, k.Score, k.Source.ToString(), k.Trusted) : null,
                }));
            }

            await resolver.RebuildAsync(form, [document], Ct);
        }

        return (lines, keyLines, replayLines, layerLines, recordLines);
    }

    /// <summary>
    /// The decision record of every document the layers replay of <paramref name="field"/> asks about, in settlement order:
    /// the key layer's best value (strength, whether it is the settled one, the key behind it, whether a value to catch held
    /// the layer back) and the similar document vote (score, whether it is the settled one, the nearest voter's similarity,
    /// whether more than one voted), whatever the thresholds — read off the replay the thresholds are chosen from.
    /// </summary>
    private static async Task<List<string>> RecordLinesAsync(FormDefinition form, string field, IReadOnlyList<SettledDocument> documents)
    {
        var replay = typeof(ThresholdSelection).GetMethod("ReplayAsync", BindingFlags.NonPublic | BindingFlags.Static)!;
        var task = (Task)replay.Invoke(null, [new FieldMemory(), new LexicalMemory(), form, field, documents, Counted, Ct, false])!;
        await task;
        var result = task.GetType().GetProperty("Result")!.GetValue(task)!;
        var steps = (IList)result.GetType().GetField("Item1")!.GetValue(result)!;
        // The replay's own order (the scalar ordinal comparer is linked into several assemblies — take the core's).
        var scalarOrdinal = (IComparer<string>)typeof(ThresholdSelection).Assembly.GetType("Gil.ScalarOrdinal")!
            .GetField("Instance", BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)!.GetValue(null)!;
        var ordered = documents
            .Select(FormResolver.Remembered)
            .OrderBy(d => d.SettledAt)
            .ThenBy(d => d.DocumentId, scalarOrdinal)
            .ToList();
        static object? Read(object step, string name) => step.GetType().GetProperty(name)!.GetValue(step);
        static object? Item(object tuple, int i) => tuple.GetType().GetField($"Item{i}")!.GetValue(tuple);
        var lines = new List<string>();
        foreach (var step in steps)
        {
            var order = (int)Read(step!, "Order")!;
            var document = ordered[order - 1];
            var key = Read(step!, "Key");
            var match = Read(step!, "Match");
            lines.Add(Line(new JsonObject
            {
                ["kind"] = "record",
                ["field"] = field,
                ["record"] = document.DocumentId,
                ["order"] = order,
                ["weight"] = (double)Read(step!, "Weight")!,
                ["key"] = key is null ? null : new JsonArray((double)Item(key, 1)!, (bool)Item(key, 2)!, (string)Item(key, 3)!, (bool)Item(key, 4)!),
                ["similar"] = match is null ? null : new JsonArray((double)Item(match, 1)!, (bool)Item(match, 2)!, (double)Item(match, 3)!, (bool)Item(match, 4)!),
                ["settled"] = document.Values[field],
                ["settlement"] = document.Settlements.TryGetValue(field, out var kind) ? kind.ToString() : null,
                ["probability"] = document.Probabilities.TryGetValue(field, out var p) ? p : null,
            }));
        }

        return lines;
    }

    private static string Line(JsonObject line) => line.ToJsonString(Json);

    private static FormDefinition With(FormDefinition form, FieldDefinition field) =>
        new(form.Name, [.. form.Fields.Select(f => f.Name == field.Name ? field : f)], form.Language);

    private static JsonArray? Strings(IReadOnlyList<string>? values) => values is null ? null : new JsonArray([.. values.Select(v => (JsonNode)v)]);

    private static JsonObject Replay(ThresholdReplay replay) => new()
    {
        ["lookups"] = replay.Lookups,
        ["candidates"] = replay.Candidates,
        ["chosen"] = Choice(replay.Chosen),
        ["most_precise"] = Choice(replay.MostPrecise),
        ["keys"] = replay.Keys is { } keys
            ? new JsonArray([.. keys.Select(k => (JsonNode)new JsonArray(k.Key, k.Lookups, k.Answered, k.Precision, k.Answering))])
            : null,
    };

    private static JsonObject? Choice(ThresholdChoice? choice) => choice is null ? null : new JsonObject
    {
        ["threshold"] = choice.Threshold,
        ["precision"] = choice.Precision,
        ["answered"] = choice.Answered,
        ["lookups"] = choice.Lookups,
        ["answered_weight"] = choice.AnsweredWeight,
        ["effective_answered"] = choice.EffectiveAnswered,
        ["similarity_floor"] = choice.SimilarityFloor,
        ["promise"] = choice.Promise,
        ["promise_answered"] = choice.PromiseAnswered,
    };

    /// <summary>How many similar documents a lookup asks for: the resolver's default of ten voters, and one more for the document itself.</summary>
    private const int Asked = 11;

    /// <summary>
    /// The similar document memory alone (lexical.jsonl): for each single-valued judged field, the first records remembered
    /// by their evidence as the resolver states it, then for every later record the documents most like its observed
    /// values, before it is remembered. Along the way some documents are forgotten and some remembered again, so the
    /// weights are retaken over changing frequencies and rows move.
    /// </summary>
    private static async Task<List<string>> LexicalAsync(List<SettledDocument> documents)
    {
        var form = Form();
        var fields = form.Fields.Where(f => f.Role == FieldRole.Judged && !f.Multiple).Select(f => f.Name).ToList();
        var memory = new LexicalMemory();

        async Task PutAsync(SettledDocument settled)
        {
            var document = FormResolver.Remembered(settled);
            foreach (var field in fields)
            {
                var task = FormResolver.TaskName(form, field);
                if (document.Values.TryGetValue(field, out var value))
                {
                    await memory.RememberAsync(task, document.DocumentId, FormResolver.Evidence(form, field, document.Values, document.Sets), value, "golden", Ct);
                }
                else
                {
                    memory.Forget(task, document.DocumentId);
                }
            }
        }

        foreach (var document in documents.Take(Chosen))
        {
            await PutAsync(document);
        }

        var lines = new List<string>();
        for (var i = Chosen; i < documents.Count; i++)
        {
            var document = documents[i];
            var observed = document.Values.Where(v => form.Field(v.Key).Role == FieldRole.Observed).ToDictionary();
            foreach (var field in fields)
            {
                var task = FormResolver.TaskName(form, field);
                lines.Add(Line(new JsonObject
                {
                    ["kind"] = "nearest",
                    ["doc"] = document.DocumentId,
                    ["field"] = field,
                    ["count"] = memory.Count(task),
                    ["matches"] = new JsonArray([.. memory.Nearest(task, FormResolver.Evidence(form, field, observed), Asked).Select(m => (JsonNode)new JsonArray(m.Source, m.Similarity, m.Answer))]),
                }));
            }

            await PutAsync(document);
            if (i % 10 == 0)
            {
                foreach (var field in fields)
                {
                    memory.Forget(FormResolver.TaskName(form, field), documents[i - 150].DocumentId);
                }
            }

            if (i % 7 == 0)
            {
                await PutAsync(documents[i - 1]); // remembered again: keeps its place among ties
            }
        }

        return lines;
    }

    /// <summary>
    /// Choosing key fields and concentrations, with every trial (key_fields.jsonl): the form path's choice for its team on
    /// the first records, and on inspection.jsonl — where a broad field alone answers more often than the field naming what
    /// the judged field describes, which the gain over the target keeps — the concentrations, then the key fields chosen
    /// with them.
    /// </summary>
    private static List<string> KeyFieldLines(List<SettledDocument> documents, List<SettledDocument> inspections)
    {
        // The typed replays only where typing was counted, so the lines of a choice without it read as before.
        static JsonObject Typed(JsonObject line, TypedKeyThresholds? typed)
        {
            if (typed is not null)
            {
                line["typed"] = new JsonArray([.. typed.ByLength.Select(r => (JsonNode)Replay(r))]);
            }

            return line;
        }

        static JsonObject Choice(string world, string field, KeyFieldsChoice choice) => Typed(new JsonObject
        {
            ["kind"] = "key_fields",
            ["world"] = world,
            ["field"] = field,
            ["fields"] = Strings(choice.KeyFields),
            ["chosen"] = Replay(choice.Chosen),
            ["all_fields"] = Replay(choice.AllFields),
            ["trials"] = new JsonArray([.. choice.Trials.Select(t => (JsonNode)Typed(new JsonObject { ["fields"] = Strings(t.KeyFields), ["replay"] = Replay(t.Replay) }, t.Typed))]),
        }, choice.Typed);

        var lines = new List<string>();
        var ticket = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Form(), "team", documents.Take(Chosen), 0.8, 10, counting: Counted);
        lines.Add(Line(Choice("ticket", "team", ticket)));

        FormDefinition Inspection(IReadOnlyDictionary<string, double>? concentrations) => new(
            "inspection",
            [
                new FieldDefinition("outlet", FieldRole.Observed),
                new FieldDefinition("kind", FieldRole.Observed),
                new FieldDefinition("inspector", FieldRole.Observed),
                new FieldDefinition("risk", FieldRole.Judged) { KeyConcentrations = concentrations },
            ],
            PromptLanguage.English);
        var concentrations = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(null), "risk", inspections);
        lines.Add(Line(new JsonObject
        {
            ["kind"] = "key_concentrations",
            ["world"] = "inspection",
            ["field"] = "risk",
            ["concentrations"] = new JsonObject([.. concentrations.OrderBy(c => c.Key, StringComparer.Ordinal).Select(c => KeyValuePair.Create(c.Key, (JsonNode?)c.Value))]),
        }));
        var inspection = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Inspection(concentrations), "risk", inspections, 0.8, 10);
        lines.Add(Line(Choice("inspection", "risk", inspection)));

        // The ticket's team again, counting the answers while its first two characters are typed as well.
        var typed = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Form(), "team", documents.Take(Chosen), 0.8, 10, typedLongest: 2, counting: Counted);
        lines.Add(Line(Choice("ticket-typed", "team", typed)));
        return lines;
    }

    /// <summary>Made-up inspections: each outlet keeps its own risk, the kind of visit mostly follows it, an inspector says nothing.</summary>
    private static IEnumerable<SettledDocument> Inspections()
    {
        var random = new Random(3);
        string[] kinds = ["routine", "complaint", "license"];
        var risks = Enumerable.Range(0, 300).Select(_ => random.Next(3)).ToArray();
        for (var i = 0; i < 800; i++)
        {
            var outlet = random.Next(risks.Length);
            var kind = random.NextDouble() < 0.85 ? kinds[risks[outlet]] : kinds[random.Next(kinds.Length)];
            yield return new SettledDocument(
                $"d{i}",
                new Dictionary<string, string> { ["outlet"] = $"o{outlet}", ["kind"] = kind, ["inspector"] = $"i{random.Next(40)}", ["risk"] = $"r{risks[outlet]}" },
                DateTimeOffset.UnixEpoch.AddMinutes(i));
        }
    }

    // --- the records: a language-neutral line per settled document

    /// <summary>A record as a JSON line; the arrival order only when the document has one, so records without it read as before.</summary>
    private static string Write(SettledDocument d)
    {
        var line = new JsonObject
        {
            ["id"] = d.DocumentId,
            ["settled_at"] = d.SettledAt.ToString("O"),
            ["values"] = new JsonObject([.. d.Values.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value))]),
            ["sets"] = new JsonObject([.. d.Sets.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)new JsonArray([.. v.Value.Select(e => (JsonNode)e)])))]),
            ["settlements"] = new JsonObject([.. d.Settlements.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value.ToString()))]),
            ["probabilities"] = new JsonObject([.. d.Probabilities.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value))]),
        };
        if (d.Arrival is { } arrival)
        {
            line["arrival"] = new JsonArray([.. arrival.Select(f => (JsonNode)f)]);
        }

        return line.ToJsonString(Json);
    }

    private static SettledDocument Read(string line)
    {
        var o = JsonNode.Parse(line)!.AsObject();
        return new SettledDocument(
            o["id"]!.GetValue<string>(),
            o["values"]!.AsObject().ToDictionary(v => v.Key, v => v.Value!.GetValue<string>()),
            DateTimeOffset.Parse(o["settled_at"]!.GetValue<string>(), System.Globalization.CultureInfo.InvariantCulture),
            o["arrival"]?.AsArray().Select(f => f!.GetValue<string>()).ToList())
        {
            Sets = o["sets"]!.AsObject().ToDictionary(v => v.Key, v => (IReadOnlyList<string>)[.. v.Value!.AsArray().Select(e => e!.GetValue<string>())]),
            Settlements = o["settlements"]!.AsObject().ToDictionary(v => v.Key, v => Enum.Parse<SettlementKind>(v.Value!.GetValue<string>())),
            Probabilities = o["probabilities"]!.AsObject().ToDictionary(v => v.Key, v => v.Value!.GetValue<double>()),
        };
    }

    /// <summary>Made-up support tickets: the product mostly decides the team, the team the code's chapter, the summary the rest.</summary>
    private static IEnumerable<SettledDocument> Generate()
    {
        string[] channels = ["email", "phone", "chat", "포털", "ｗｅｂ"];
        string[] products = ["P-10", "P-11", "P-20", "P-21", "P-30", "P-31", "P-40", "Q-50", "Q-51", "Q-60", "Q-70", "Q-80"];
        string[] teams = ["billing", "network", "devices", "accounts", "logistics", "security"];
        string[][] codes = [["21-10", "21-20", "21-30"], ["22-10", "22-40"], ["23-10", "23-20", "23-50"], ["24-10", "24-20"], ["25-30", "25-40"], ["26-10", "26-60"]];
        string[] phrases =
        [
            "결제가 두 번 되었습니다", "환불 요청", "인터넷 연결이 끊김", "공유기 재부팅 안 됨", "로그인 비밀번호 분실",
            "ÖDEME İPTAL edildi", "ΣΦΑΛΜΑ ΣΥΝΔΕΣΗΣ", "ｆｕｌｌｗｉｄｔｈ ｔｅｘｔ", "device 😀 overheats", "naïve café order lost",
            "parcel 📦 delayed", "suspicious login 🔐 alert", "invoice copy please", "router lights blinking",
        ];
        string[] tagPool = ["urgent", "vip", "repeat", "refund", "outage", "hardware", "fraud", "kr"];
        var random = new Random(20261016);
        var at = new DateTimeOffset(2026, 1, 1, 0, 0, 0, TimeSpan.Zero);
        for (var i = 1; i <= 400; i++)
        {
            var product = products[random.Next(products.Length)];
            var t = random.NextDouble() < 0.85 ? Array.IndexOf(products, product) % teams.Length : random.Next(teams.Length);
            var code = codes[t][random.Next(codes[t].Length)];
            var summary = $"{phrases[(t * 2 + random.Next(3)) % phrases.Length]} {phrases[random.Next(phrases.Length)]}";
            var tags = tagPool.Where((_, k) => random.NextDouble() < (k == t ? 0.6 : 0.12)).ToList();
            var kind = (i % 11, i % 13) switch
            {
                (0, _) => SettlementKind.Fill,
                (_, 0) => SettlementKind.Check,
                _ => (i % 3) switch { 0 => SettlementKind.Enter, 1 => SettlementKind.Accept, _ => SettlementKind.Correct },
            };
            yield return new SettledDocument(
                $"t-{i:D4}",
                new Dictionary<string, string> { ["channel"] = channels[random.Next(channels.Length)], ["product"] = product, ["summary"] = summary, ["team"] = teams[t], ["code"] = code },
                at.AddMinutes(i * 7))
            {
                Sets = tags.Count > 0 ? new Dictionary<string, IReadOnlyList<string>> { ["tags"] = tags } : new Dictionary<string, IReadOnlyList<string>>(),
                Settlements = new Dictionary<string, SettlementKind> { ["team"] = kind, ["code"] = kind == SettlementKind.Fill ? SettlementKind.Enter : kind },
                Probabilities = kind == SettlementKind.Check ? new Dictionary<string, double> { ["team"] = 0.25, ["code"] = 0.25 } : new Dictionary<string, double>(),
            };
        }
    }
}
