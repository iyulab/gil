using System.Text.Json.Nodes;
using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>
/// The decision core's second golden (tests/fixtures/core/note-*.jsonl): a form whose long free text decides a judged
/// field through the similar document layer, while the key layer is held back by a value that must not be missed, a
/// value settled outside the field's candidates, a coarse level made from a table, a field that takes several values
/// trusted on its own scores, typed thresholds, values that arrive in different orders — and, among the decisions,
/// documents settled again and copied under new ids, so the resolver's cases are rewritten while it is asked.
/// </summary>
public sealed partial class CoreGoldenTests
{
    private const int NoteChosen = 240; // settings are chosen on the first notes, decisions made on the rest

    private static readonly string[] Topics = ["net-wifi", "net-cable", "pay-card", "pay-bank", "acct-login", "acct-delete", "fraud"];

    private static FormDefinition NoteForm() => new("note",
    [
        new FieldDefinition("desk", FieldRole.Observed),
        new FieldDefinition("body", FieldRole.Observed),
        new FieldDefinition("ref", FieldRole.Observed),
        new FieldDefinition("topic", FieldRole.Judged)
        {
            DependsOn = ["body", "ref"],
            Candidates = Topics,
            MustCatch = ["fraud"],
            Coarse = new CoarseLevel(new Dictionary<string, string>
            {
                ["net-wifi"] = "network", ["net-cable"] = "network",
                ["pay-card"] = "payment", ["pay-bank"] = "payment",
                ["acct-login"] = "account", ["acct-delete"] = "account",
            }),
        },
        new FieldDefinition("labels", FieldRole.Judged) { Multiple = true, DependsOn = ["desk"] },
        new FieldDefinition("owner", FieldRole.Judged) { DependsOn = ["desk", "ref"] },
    ], PromptLanguage.English);

    [Fact]
    public async Task The_core_reproduces_its_note_golden()
    {
        var directory = Path.Combine(AppContext.BaseDirectory, "core");
        if (Environment.GetEnvironmentVariable("GIL_WRITE_CORE_GOLDEN") is { } target)
        {
            Directory.CreateDirectory(target);
            await File.WriteAllLinesAsync(Path.Combine(target, "note-records.jsonl"), Notes().Select(Write), Ct);
            var written = await NoteAsync([.. (await File.ReadAllLinesAsync(Path.Combine(target, "note-records.jsonl"), Ct)).Select(Read)]);
            await File.WriteAllLinesAsync(Path.Combine(target, "note-golden.jsonl"), written, Ct);
            return;
        }

        var documents = (await File.ReadAllLinesAsync(Path.Combine(directory, "note-records.jsonl"), Ct)).Select(Read).ToList();
        (await NoteAsync(documents)).Should().Equal(await File.ReadAllLinesAsync(Path.Combine(directory, "note-golden.jsonl"), Ct));
    }

    /// <summary>
    /// The settings chosen on the first notes, each as a line, then for every later note: a suggestion for every judged
    /// field from its observed values, the set field again with its first value chosen, and the owner while its first one
    /// and two characters are typed — before the note is put; every fourth note is asked about again once put, as a saved
    /// document is. Every sixth note also settles an early note again with
    /// another topic, every tenth copies one under a new id, each later than all before.
    /// </summary>
    private static async Task<List<string>> NoteAsync(IReadOnlyList<SettledDocument> documents)
    {
        var form = NoteForm();
        var early = documents.Take(NoteChosen).ToList();
        var lines = new List<string>();

        var topic = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "topic", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "layers", ["field"] = "topic", ["key"] = Replay(topic.Key), ["memory"] = Replay(topic.Memory) }));
        form = With(form, form.Field("topic") with
        {
            KeyThreshold = topic.Key.Chosen?.Threshold,
            MemoryThreshold = topic.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = topic.Memory.Chosen?.SimilarityFloor,
        });

        var coarse = await ThresholdSelection.SelectCoarseAsync(new FieldMemory(), new LexicalMemory(), form, "topic", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "coarse", ["field"] = "topic", ["key"] = Replay(coarse.Key), ["memory"] = Replay(coarse.Memory) }));
        form = With(form, form.Field("topic") with
        {
            Coarse = form.Field("topic").Coarse! with
            {
                KeyThreshold = coarse.Key.Chosen?.Threshold,
                MemoryThreshold = coarse.Memory.Chosen?.Threshold,
                MemorySimilarityFloor = coarse.Memory.Chosen?.SimilarityFloor,
            },
        });

        var labels = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), form, "labels", early, 0.8, 10);
        lines.Add(Line(new JsonObject { ["kind"] = "key", ["field"] = "labels", ["replay"] = Replay(labels) }));
        form = With(form, form.Field("labels") with { KeyThreshold = labels.Chosen?.Threshold });

        var owner = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "owner", early, 0.8, 10, Counted, Ct);
        lines.Add(Line(new JsonObject { ["kind"] = "layers", ["field"] = "owner", ["key"] = Replay(owner.Key), ["memory"] = Replay(owner.Memory) }));
        form = With(form, form.Field("owner") with
        {
            KeyThreshold = owner.Key.Chosen?.Threshold,
            MemoryThreshold = owner.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = owner.Memory.Chosen?.SimilarityFloor,
        });

        var typed = ThresholdSelection.SelectTypedKeyThresholds(new FieldMemory(), form, "owner", early, 0.8, 10, longest: 2, counting: Counted);
        lines.Add(Line(new JsonObject { ["kind"] = "typed", ["field"] = "owner", ["replays"] = new JsonArray([.. typed.ByLength.Select(r => (JsonNode)Replay(r))]) }));
        form = With(form, form.Field("owner") with { TypedKeyThresholds = typed.Thresholds });

        JsonObject Decision(string kind, string id, FieldSuggestion suggestion) => new()
        {
            ["kind"] = kind,
            ["doc"] = id,
            ["field"] = suggestion.Field,
            ["answered"] = suggestion.Answered,
            ["source"] = suggestion.Source.ToString(),
            ["candidates"] = new JsonArray([.. suggestion.Candidates.Take(3).Select(c => (JsonNode)new JsonArray(c.Value, c.Score, c.Source.ToString(), c.Evidence, c.Trusted))]),
            ["coarse"] = suggestion.Coarse is { } k ? new JsonArray(k.Value, k.Score, k.Source.ToString(), k.Evidence, k.Trusted) : null,
        };

        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        await resolver.RebuildAsync(form, early, Ct);
        var latest = documents[^1].SettledAt;
        var later = documents.Skip(NoteChosen).ToList();
        for (var j = 0; j < later.Count; j++)
        {
            var document = later[j];
            var observed = document.Values.Where(v => form.Field(v.Key).Role == FieldRole.Observed).ToDictionary();
            foreach (var suggestion in await resolver.SuggestAsync(form, document.DocumentId, observed, Ct))
            {
                lines.Add(Line(Decision("decision", document.DocumentId, suggestion)));
            }

            if (document.Sets.TryGetValue("labels", out var settled) && settled.Count > 0)
            {
                var sets = new Dictionary<string, IReadOnlyList<string>> { ["labels"] = [settled[0]] };
                var chosen = await resolver.SuggestAsync(form, document.DocumentId, "labels", observed, sets, cancellationToken: Ct);
                lines.Add(Line(Decision("set_decision", document.DocumentId, chosen).Also("chosen", settled[0])));
            }

            foreach (var length in new[] { 1, 2 })
            {
                var text = document.Values["owner"][..length];
                var suggestion = await resolver.SuggestAsync(form, document.DocumentId, "owner", observed, typed: text, cancellationToken: Ct);
                lines.Add(Line(Decision("typed_decision", document.DocumentId, suggestion).Also("typed", text)));
            }

            await resolver.RebuildAsync(form, [document], Ct);

            // Asked again once saved, as an editor asks about a document it reopens: its own settlement is no evidence.
            if (j % 4 == 0)
            {
                foreach (var suggestion in await resolver.SuggestAsync(form, document.DocumentId, observed, Ct))
                {
                    lines.Add(Line(Decision("saved_decision", document.DocumentId, suggestion)));
                }
            }

            if (j % 6 == 5)
            {
                var again = early[j];
                var moved = Topics[(Array.IndexOf(Topics, again.Values["topic"]) + 1) % (Topics.Length - 1)];
                latest = latest.AddMinutes(1);
                await resolver.RebuildAsync(form, [again with { Values = new Dictionary<string, string>(again.Values) { ["topic"] = moved }, SettledAt = latest }], Ct);
            }

            if (j % 10 == 9)
            {
                latest = latest.AddMinutes(1);
                await resolver.RebuildAsync(form, [early[j * 2] with { DocumentId = $"{early[j * 2].DocumentId}-copy", SettledAt = latest }], Ct);
            }
        }

        return lines;
    }

    /// <summary>
    /// Made-up support notes: the long body decides the topic (rarely fraud, or a retired topic outside the candidates)
    /// or at least its class,
    /// the desk the labels and mostly the owner, the reference is noise but for one that fraud favours. Values arrive in
    /// three orders: the body first and the topic before the rest, unknown, and the form's.
    /// </summary>
    private static IEnumerable<SettledDocument> Notes()
    {
        string[][] phrases =
        [
            ["wifi drops every evening", "the wireless signal is weak upstairs", "cannot see the home network name"],
            ["the cable modem shows a red light", "wired connection stays offline", "ethernet port seems dead"],
            ["the card payment was declined", "charged twice on my credit card", "card expired during checkout"],
            ["the bank transfer did not arrive", "direct debit failed this month", "wrong account number on transfer"],
            ["cannot log in after the update", "password reset email never came", "two factor code is rejected"],
            ["please delete my account and data", "close my profile permanently", "remove every record of me"],
            ["someone used my account abroad", "unknown purchases on my statement", "phishing message asked for my pin"],
        ];
        string[] fillers = ["thanks in advance", "this is urgent", "Grüße aus Köln", "고객센터에 문의드립니다", "since yesterday", "über das Wochenende"];
        string[] desks = ["d-north", "d-south", "d-east", "d-west", "d-hq", "d-ops"];
        string[][] deskLabels = [["home", "evening"], ["business"], ["home", "vip"], ["mobile"], ["vip", "legal"], ["business", "mobile"]];
        string[] labelPool = ["home", "evening", "business", "vip", "mobile", "legal"];
        string[] owners = ["ana", "ben", "bora", "cho", "chae", "dee"];
        var random = new Random(20261018);
        var at = new DateTimeOffset(2026, 3, 1, 9, 0, 0, TimeSpan.FromHours(9));
        for (var i = 1; i <= 340; i++)
        {
            var roll = random.NextDouble();
            var t = roll < 0.04 ? 6 : random.Next(6);
            var topic = roll is >= 0.04 and < 0.07 ? "legacy" : Topics[t];
            // A third of the bodies also read like the sibling topic of the same class, some like any topic: the vote on the
            // value splits where the vote on its class does not.
            var mix = random.NextDouble();
            var second = mix < 0.35 && t < 6 ? t ^ 1 : mix < 0.45 ? random.Next(7) : t;
            var body = $"{phrases[t][random.Next(3)]}; {phrases[second][random.Next(3)]} — {fillers[random.Next(fillers.Length)]}";
            var desk = random.Next(desks.Length);
            var labels = labelPool.Where(l => random.NextDouble() < (deskLabels[desk].Contains(l) ? 0.8 : 0.08)).ToList();
            var owner = random.NextDouble() < 0.8 ? owners[desk] : owners[random.Next(owners.Length)];
            var reference = t == 6 && random.NextDouble() < 0.5 ? "r-7" : $"r-{random.Next(25)}";
            var kind = (i % 11, i % 13) switch
            {
                (0, _) => SettlementKind.Fill,
                (_, 0) => SettlementKind.Check,
                _ => (i % 3) switch { 0 => SettlementKind.Enter, 1 => SettlementKind.Accept, _ => SettlementKind.Correct },
            };
            IReadOnlyList<string>? arrival = (i % 4) switch
            {
                0 => ["body", "topic", "desk", "ref", "owner"],
                1 => null,
                _ => ["desk", "ref", "body", "owner", "topic"],
            };
            yield return new SettledDocument(
                $"n-{i:D4}",
                new Dictionary<string, string> { ["desk"] = desks[desk], ["body"] = body, ["ref"] = reference, ["topic"] = topic, ["owner"] = owner },
                at.AddMinutes(i * 11),
                arrival)
            {
                Sets = labels.Count > 0 ? new Dictionary<string, IReadOnlyList<string>> { ["labels"] = labels } : new Dictionary<string, IReadOnlyList<string>>(),
                Settlements = new Dictionary<string, SettlementKind> { ["topic"] = kind, ["owner"] = kind == SettlementKind.Fill ? SettlementKind.Enter : kind },
                Probabilities = kind == SettlementKind.Check ? new Dictionary<string, double> { ["topic"] = 0.25, ["owner"] = 0.25 } : new Dictionary<string, double>(),
            };
        }
    }
}

internal static class GoldenLines
{
    /// <summary>The line with one more string member.</summary>
    public static JsonObject Also(this JsonObject line, string name, string value)
    {
        line[name] = value;
        return line;
    }
}
