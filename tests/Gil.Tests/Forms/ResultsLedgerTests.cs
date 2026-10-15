using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>
/// What a replay reads off the answers it counted: the promise over the latest documents, and the state under each key.
/// </summary>
public sealed class ResultsLedgerTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    private static readonly FormDefinition Ticket = new(
        "ticket",
        [
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.3 },
        ],
        PromptLanguage.English);

    private static readonly DateTimeOffset Monday = new(2026, 9, 28, 9, 0, 0, TimeSpan.Zero);

    private static SettledDocument Doc(int n, string? summary, string team) =>
        new(
            $"d{n:D5}",
            summary is null ? new Dictionary<string, string> { ["team"] = team } : new Dictionary<string, string> { ["summary"] = summary, ["team"] = team },
            Monday.AddMinutes(n));

    /// <summary>
    /// Ten summaries, each settled with its own team; from <paramref name="drift"/> on, about one in six goes elsewhere,
    /// whatever the summary — so each still backs its team strongly, and the threshold keeps answering it.
    /// </summary>
    private static List<SettledDocument> Stream(int count, int drift) =>
        [.. Enumerable.Range(1, count).Select(n => Doc(n, $"case {n % 10}", n >= drift && n * 7_919 % 101 < 17 ? "elsewhere" : $"team {n % 10}"))];

    [Fact]
    public void A_short_replay_promises_its_precision()
    {
        var chosen = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", Stream(400, drift: 200), 0.5, 30).Chosen!;
        chosen.Promise.Should().Be(chosen.Precision); // every document is among the latest
        chosen.PromiseAnswered.Should().Be(chosen.EffectiveAnswered);
    }

    [Fact]
    public void The_promise_follows_the_latest_documents_when_the_layer_has_changed()
    {
        // Right throughout the first 6,000; a quarter wrong after. The whole replay still remembers the good stretch.
        // Every summary still backs its team, so every lookup after the first ten is answered with it.
        var documents = Stream(9_000, drift: 6_000);
        static double Right(IEnumerable<SettledDocument> ds) => ds.Average(d => d.Values["team"] == "elsewhere" ? 0.0 : 1.0);
        var chosen = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 30).Chosen!;
        chosen.Answered.Should().Be(9_000 - 10);
        chosen.Precision.Should().BeApproximately(Right(documents.Skip(10)), 1e-12);
        chosen.Promise.Should().BeApproximately(Right(documents.Skip(9_000 - ThresholdSelection.RecentDocuments)), 1e-12);
        chosen.Promise.Should().BeLessThan(chosen.Precision - 0.04);
        chosen.PromiseAnswered.Should().Be(ThresholdSelection.RecentDocuments);
    }

    [Fact]
    public void Too_few_recent_answers_leave_the_promise_at_the_whole_replays_precision()
    {
        // The latest 5,000 documents carry no summary, so no key answers them; the promise has nothing recent to rest on.
        var documents = Stream(2_000, drift: 1_000).Concat(Enumerable.Range(2_001, ThresholdSelection.RecentDocuments).Select(n => Doc(n, null, "team 1"))).ToList();
        var chosen = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", documents, 0.5, 30).Chosen!;
        chosen.Promise.Should().Be(chosen.Precision);
        chosen.PromiseAnswered.Should().Be(chosen.EffectiveAnswered);
    }

    private static readonly SettledDocument[] TwoKeys =
    [
        Doc(1, "vpn drops", "network"),
        Doc(2, "printer jam", "facilities"),
        Doc(3, "vpn drops", "network"),
        Doc(4, "printer jam", "facilities"),
        Doc(5, "vpn drops", "network"),
        Doc(6, "printer jam", "network"),
    ];

    [Fact]
    public void Each_key_says_how_the_chosen_threshold_answered_under_it()
    {
        // Each key's first document has nothing under it; the next two are answered — both right under one key, one under the other.
        var replay = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", TwoKeys, 0.5, 1);
        replay.Chosen!.Answered.Should().Be(4);
        replay.Keys.Should().BeEquivalentTo(
            [
                new KeyState("summary: printer jam", 2, 2, 0.5, true) { AnsweredWeight = 2, EffectiveAnswered = 2 },
                new KeyState("summary: vpn drops", 2, 2, 1.0, true) { AnsweredWeight = 2, EffectiveAnswered = 2 },
            ],
            o => o.WithStrictOrdering());
    }

    [Fact]
    public void With_no_threshold_chosen_no_key_answers()
    {
        var replay = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Ticket, "team", TwoKeys, 0.99, 1);
        replay.Chosen.Should().BeNull();
        replay.Keys!.Should().AllSatisfy(k =>
        {
            k.Lookups.Should().Be(2);
            (k.Answered, k.Precision, k.Answering).Should().Be((0, (double?)null, false));
        });
    }

    [Fact]
    public async Task Only_the_key_layer_carries_key_states()
    {
        var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), Ticket, "team", TwoKeys, 0.5, 1, Ct);
        layers.Key.Keys.Should().HaveCount(2);
        layers.Memory.Keys.Should().BeNull();
    }
}
