using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

/// <summary>Which observed field to ask a person for first (<see cref="AskPlanner"/>).</summary>
public sealed class AskPlannerTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    // The part names the code; the note is free text no two reports share, so without the part nothing answers.
    private static FormDefinition Report(CoarseLevel? coarse = null) => new(
        "report",
        [
            new FieldDefinition("part", FieldRole.Observed),
            new FieldDefinition("note", FieldRole.Observed),
            new FieldDefinition("code", FieldRole.Judged) { KeyThreshold = 0.5, Coarse = coarse },
        ],
        PromptLanguage.English);

    private static SettledDocument Doc(int number, string part, string code) =>
        new($"d{number}", new Dictionary<string, string> { ["part"] = part, ["note"] = $"note {number}", ["code"] = code },
            DateTimeOffset.UnixEpoch.AddMinutes(number));

    private static async Task<(AskPlanner Planner, FormResolver Resolver)> LearnAsync(FormDefinition form, IEnumerable<SettledDocument> documents, double budget = 0.5)
    {
        var resolver = new FormResolver(new FieldMemory());
        var planner = new AskPlanner(form, "code", ["part", "note"], budget);
        foreach (var document in documents)
        {
            await planner.LearnAsync(resolver, document, Ct);
            await resolver.RebuildAsync(form, [document], Ct);
        }

        return (planner, resolver);
    }

    [Fact]
    public async Task Asking_the_field_that_names_the_value_gains_per_character_typed()
    {
        // Six pump reports: from the second on, the part alone answers the code; without it nothing does.
        var documents = Enumerable.Range(1, 6).Select(n => Doc(n, "pump", "P1")).ToList();
        var (planner, resolver) = await LearnAsync(Report(), documents);

        var open = new Dictionary<string, string> { ["note"] = "note 9" };
        var suggestion = await resolver.SuggestAsync(Report(), "d9", "code", open, cancellationToken: Ct);
        suggestion.Answered.Should().BeFalse();

        var advice = planner.Advise(suggestion, open);

        // Learned under "no class shown": five of six right after asking, four characters each.
        advice.Should().ContainSingle();
        advice[0].Field.Should().Be("part");
        advice[0].Gain.Should().BeApproximately((5 + AskPlanner.PriorGain) / ((6 * 4) + AskPlanner.PriorCharacters), 1e-12);
        advice[0].Threshold.Should().NotBeNull();
    }

    [Fact]
    public async Task Fields_are_ranked_by_gain_and_a_field_with_a_value_is_left_out()
    {
        var documents = Enumerable.Range(1, 6).Select(n => Doc(n, "pump", "P1")).ToList();
        var (planner, resolver) = await LearnAsync(Report(), documents);
        var none = new Dictionary<string, string>();
        var suggestion = await resolver.SuggestAsync(Report(), "d9", "code", none, cancellationToken: Ct);

        var advice = planner.Advise(suggestion, none);

        // The note never helped (each is new), so it ranks below the part with only its prior.
        advice.Select(a => a.Field).Should().Equal("part", "note");
        advice[1].Gain.Should().BeLessThan(advice[0].Gain);
        advice[1].Ask.Should().BeFalse();

        planner.Advise(suggestion, new Dictionary<string, string> { ["part"] = "pump" }).Select(a => a.Field).Should().Equal("note");
    }

    [Fact]
    public async Task A_field_the_suggestion_already_answers_is_not_worth_asking()
    {
        var documents = Enumerable.Range(1, 6).Select(n => Doc(n, "pump", "P1")).ToList();
        var (planner, resolver) = await LearnAsync(Report(), documents);
        var open = new Dictionary<string, string> { ["part"] = "pump" };
        var answered = await resolver.SuggestAsync(Report(), "d9", "code", open, cancellationToken: Ct);
        answered.Answered.Should().BeTrue();

        planner.Advise(answered, open).Should().OnlyContain(a => !a.Ask);
    }

    [Fact]
    public async Task A_trusted_class_holding_the_value_counts_the_share_of_values_it_rules_out()
    {
        // Two chapters of two codes each; the note's word backs the chapter, the part names the code.
        var table = new Dictionary<string, string> { ["2710"] = "27", ["2711"] = "27", ["3230"] = "32", ["3250"] = "32" };
        var form = Report(new CoarseLevel(table) { KeyThreshold = 0.5 });
        SettledDocument Report2(int n, string part, string word, string code) =>
            new($"d{n}", new Dictionary<string, string> { ["part"] = part, ["note"] = word, ["code"] = code }, DateTimeOffset.UnixEpoch.AddMinutes(n));
        var documents = new List<SettledDocument>
        {
            Report2(1, "aileron", "flight", "2710"),
            Report2(2, "elevator", "flight", "2711"),
            Report2(3, "aileron", "flight", "2710"),
        };
        var (planner, resolver) = await LearnAsync(form, documents);

        var open = new Dictionary<string, string> { ["note"] = "flight" };
        var suggestion = await resolver.SuggestAsync(form, "d9", "code", open, cancellationToken: Ct);
        suggestion.Coarse.Should().NotBeNull();
        suggestion.Coarse!.Trusted.Should().BeTrue();

        // Under "chapter 27, trusted" — what the open document shows — only document 3 was learned: without the part the
        // chapter holds 2710 (a share of log(4/2) / log 4 = 0.5), with it 2710 is answered (1), and "aileron" is 7
        // characters. Before document 3 one flight report was not enough to trust the chapter, so document 2 was learned
        // under another class.
        var first = new FormResolver(new FieldMemory());
        await first.RebuildAsync(form, documents.Take(1), Ct);
        (await first.SuggestAsync(form, "d2", "code", open, cancellationToken: Ct)).Coarse?.Trusted.Should().NotBe(true);

        var advice = planner.Advise(suggestion, open).Single(a => a.Field == "part");
        advice.Gain.Should().BeApproximately((1 - 0.5 + AskPlanner.PriorGain) / (7 + AskPlanner.PriorCharacters), 1e-12);
    }

    [Fact]
    public void The_budget_rule_asks_as_the_registered_rule_does()
    {
        // A fixed sequence of (class, trusted, gain, characters), and the asks the registered rule's code
        // (experiments/t3_ask_value.py, simulate(policy="budget"), budget 0.4) makes on it.
        (string Class, bool Trusted, double Gain, int Characters)[] sequence =
        [
            ("", false, 1.0, 4), ("", false, 0.0, 11), ("", false, 0.0, 3), ("A", true, 1.0, 9), ("A", false, 0.0, 4),
            ("", false, 0.0, 9), ("A", false, -0.25, 4), ("A", false, 0.0, 1), ("A", false, 0.0, 12), ("B", true, 0.5, 7),
            ("B", true, 0.0, 11), ("", false, 0.0, 6), ("A", false, 0.0, 12), ("A", true, -0.25, 4), ("", false, 0.0, 8),
            ("B", true, 0.0, 5), ("A", false, 0.0, 1), ("B", true, 0.0, 4), ("A", true, 0.5, 12), ("A", false, 0.0, 9),
            ("", false, -0.25, 11), ("", false, -0.25, 3), ("A", false, 0.5, 7), ("A", false, 0.0, 5), ("", false, 1.0, 8),
            ("", false, 0.5, 8), ("A", false, 0.0, 1), ("A", true, -0.25, 4), ("A", true, -0.25, 9), ("A", false, 1.0, 3),
            ("A", true, 0.0, 11), ("A", false, 0.5, 9), ("", false, -0.25, 3), ("B", true, 0.0, 12), ("B", true, 1.0, 5),
            ("", false, 0.5, 9), ("A", false, 0.5, 8), ("B", true, 0.0, 8), ("A", true, 1.0, 8), ("", false, 0.0, 11),
            ("A", true, 0.5, 9), ("", false, -0.25, 7), ("A", true, 0.0, 8), ("A", false, -0.25, 1), ("", false, 1.0, 8),
            ("", false, 0.5, 1), ("", false, 1.0, 1), ("B", true, 0.5, 9), ("", false, -0.25, 3), ("A", true, 0.0, 11),
            ("A", true, 0.0, 4), ("B", true, 1.0, 9), ("A", false, 0.0, 9), ("B", true, 0.5, 2), ("A", false, -0.25, 5),
            ("A", true, 0.0, 5), ("", false, 0.0, 6), ("", false, 1.0, 1), ("A", false, 0.5, 7), ("A", false, 0.0, 8),
        ];
        const string Expected = "010000000010010000100000010110001001110110110111110001001100";

        var ledger = new AskPlanner.Ledger();
        var asks = string.Concat(sequence.Select(s => ledger.Learn((s.Class.Length == 0 ? null : s.Class, s.Trusted), s.Gain, s.Characters, 0.4) ? '1' : '0'));

        asks.Should().Be(Expected);
    }

    [Fact]
    public void The_planner_takes_a_judged_field_observed_fields_to_ask_and_a_share_as_its_budget()
    {
        var form = Report();
        FluentActions.Invoking(() => new AskPlanner(form, "code", ["part"], 0)).Should().Throw<ArgumentOutOfRangeException>();
        FluentActions.Invoking(() => new AskPlanner(form, "code", ["part"], 1.5)).Should().Throw<ArgumentOutOfRangeException>();
        FluentActions.Invoking(() => new AskPlanner(form, "part", ["note"], 0.5)).Should().Throw<ArgumentException>();
        FluentActions.Invoking(() => new AskPlanner(form, "code", ["code"], 0.5)).Should().Throw<ArgumentException>();
        FluentActions.Invoking(() => new AskPlanner(form, "code", ["part", "part"], 0.5)).Should().Throw<ArgumentException>();
        FluentActions.Invoking(() => new AskPlanner(form, "code", [], 0.5)).Should().Throw<ArgumentException>();
        new AskPlanner(form, "code", ["part"], 1).Budget.Should().Be(1);
    }
}
