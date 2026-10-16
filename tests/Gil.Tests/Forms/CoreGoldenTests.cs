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
/// U+FFFF. Set GIL_WRITE_CORE_GOLDEN to rewrite both files from this implementation.
/// </summary>
public sealed class CoreGoldenTests
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
            await File.WriteAllLinesAsync(Path.Combine(target, "golden.jsonl"), await RunAsync([.. (await File.ReadAllLinesAsync(Path.Combine(target, "records.jsonl"), Ct)).Select(Read)]), Ct);
            return;
        }

        var documents = (await File.ReadAllLinesAsync(records, Ct)).Select(Read).ToList();

        var lines = await RunAsync(documents);

        lines.Should().Equal(await File.ReadAllLinesAsync(golden, Ct));
    }

    /// <summary>The settings chosen on the first records, then a suggestion for every judged field of every later record before it is put.</summary>
    private static async Task<List<string>> RunAsync(IReadOnlyList<SettledDocument> documents)
    {
        var form = Form();
        var early = documents.Take(Chosen).ToList();
        var lines = new List<string>();

        var keyFields = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "team", early, 0.8, 10, counting: Counted);
        lines.Add(Line(new JsonObject { ["kind"] = "key_fields", ["field"] = "team", ["fields"] = Strings(keyFields.KeyFields), ["chosen"] = Replay(keyFields.Chosen) }));
        form = With(form, form.Field("team") with { KeyFields = keyFields.KeyFields });

        var team = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "layers", ["field"] = "team", ["key"] = Replay(team.Key), ["memory"] = Replay(team.Memory) }));
        form = With(form, form.Field("team") with
        {
            KeyThreshold = team.Key.Chosen?.Threshold,
            MemoryThreshold = team.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = team.Memory.Chosen?.SimilarityFloor,
        });

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

    // --- the records: a language-neutral line per settled document

    private static string Write(SettledDocument d) => new JsonObject
    {
        ["id"] = d.DocumentId,
        ["settled_at"] = d.SettledAt.ToString("O"),
        ["values"] = new JsonObject([.. d.Values.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value))]),
        ["sets"] = new JsonObject([.. d.Sets.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)new JsonArray([.. v.Value.Select(e => (JsonNode)e)])))]),
        ["settlements"] = new JsonObject([.. d.Settlements.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value.ToString()))]),
        ["probabilities"] = new JsonObject([.. d.Probabilities.OrderBy(v => v.Key, StringComparer.Ordinal).Select(v => KeyValuePair.Create(v.Key, (JsonNode?)v.Value))]),
    }.ToJsonString(Json);

    private static SettledDocument Read(string line)
    {
        var o = JsonNode.Parse(line)!.AsObject();
        return new SettledDocument(
            o["id"]!.GetValue<string>(),
            o["values"]!.AsObject().ToDictionary(v => v.Key, v => v.Value!.GetValue<string>()),
            DateTimeOffset.Parse(o["settled_at"]!.GetValue<string>(), System.Globalization.CultureInfo.InvariantCulture))
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
