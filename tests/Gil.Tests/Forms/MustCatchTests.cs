using AwesomeAssertions;
using Gil.Forms;

namespace Gil.Tests.Forms;

/// <summary>
/// A value the field must not miss, once settled under a key, keeps the key layer from answering under that key — the
/// rare dangerous value a threshold cannot hold back.
/// </summary>
public sealed class MustCatchTests
{
    private static readonly DateTimeOffset Monday = new(2026, 9, 28, 9, 0, 0, TimeSpan.Zero);

    private static FormDefinition Log(IReadOnlyList<string>? mustCatch) => new(
        "log",
        [
            new FieldDefinition("template", FieldRole.Observed),
            new FieldDefinition("risk", FieldRole.Judged) { KeyThreshold = 0.5, MustCatch = mustCatch },
        ],
        PromptLanguage.English);

    private static SettledDocument Line(int n, string template, string risk) =>
        new($"l{n}", new Dictionary<string, string> { ["template"] = template, ["risk"] = risk }, Monday.AddMinutes(n));

    // Under t1 a 3 was settled once among many 1s; under t2 only 1s.
    private static readonly SettledDocument[] Lines =
    [
        .. Enumerable.Range(1, 20).Select(n => Line(n, "t1", "1")),
        Line(21, "t1", "3"),
        .. Enumerable.Range(22, 20).Select(n => Line(n, "t1", "1")),
        .. Enumerable.Range(42, 10).Select(n => Line(n, "t2", "1")),
    ];

    private static IReadOnlyList<FieldCandidate> Ranked(FormDefinition form, string template)
    {
        var keys = new FieldMemory();
        foreach (var line in Lines)
        {
            keys.Put(form, line);
        }

        return keys.Rank(form, "risk", new Dictionary<string, string> { ["template"] = template }, 3);
    }

    [Fact]
    public void A_key_that_once_settled_a_value_to_catch_does_not_answer()
    {
        var caught = Ranked(Log(["3", "4", "5"]), "t1");
        caught[0].Value.Should().Be("1");
        caught[0].Trusted.Should().BeFalse(); // still offered, as a guess
        caught.Select(c => c.Value).Should().Contain("3");

        Ranked(Log(["3", "4", "5"]), "t2")[0].Trusted.Should().BeTrue(); // a key that never settled one answers as before
        Ranked(Log(null), "t1")[0].Trusted.Should().BeTrue(); // the threshold alone lets the rare 3 through
        Ranked(Log([]), "t1")[0].Trusted.Should().BeTrue();
    }

    [Fact]
    public void Choosing_the_key_threshold_does_not_count_answers_the_key_layer_would_not_give()
    {
        var plain = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Log(null), "risk", Lines, 0.5, 1);
        var watched = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Log(["3"]), "risk", Lines, 0.5, 1);

        watched.Lookups.Should().Be(plain.Lookups); // every line is still asked about
        // the 20 t1 lines after the 3 are not the key layer's to answer any more
        (plain.Candidates - watched.Candidates).Should().Be(20);
    }

    [Fact]
    public void Only_a_judged_field_with_a_single_value_names_values_to_catch()
    {
        FluentActions.Invoking(() => new FormDefinition(
            "log",
            [
                new FieldDefinition("template", FieldRole.Observed) { MustCatch = ["3"] },
                new FieldDefinition("risk", FieldRole.Judged),
            ],
            PromptLanguage.English)).Should().Throw<ArgumentException>();

        FluentActions.Invoking(() => new FormDefinition(
            "log",
            [
                new FieldDefinition("template", FieldRole.Observed),
                new FieldDefinition("tags", FieldRole.Judged) { Multiple = true, MustCatch = ["3"] },
            ],
            PromptLanguage.English)).Should().Throw<ArgumentException>();
    }
}
