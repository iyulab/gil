using AwesomeAssertions;
using Gil.Forms;

namespace Gil.Tests.Forms;

/// <summary>
/// How readily each key field's keys are believed after only a few settlements, chosen from settled documents: a field
/// naming the very thing the judged field describes keeps the value its keys were first seen with, so a key of it seen
/// once already speaks for that value over a broad field whose keys mix values — and without concentrations the field
/// memory scores exactly as it always did.
/// </summary>
public sealed class KeyConcentrationTests
{
    private static readonly string[] Risks = ["high", "medium", "low"];

    // Each place has one risk, drawn mostly high; each visit has a kind drawn from a few. The kind does not decide the
    // risk — its keys mix all three — while a place seen once already names it.
    private static FormDefinition Inspection(IReadOnlyDictionary<string, double>? concentrations = null) => new(
        "inspection",
        [
            new FieldDefinition("place", FieldRole.Observed),
            new FieldDefinition("kind", FieldRole.Observed),
            new FieldDefinition("risk", FieldRole.Judged) { KeyConcentrations = concentrations },
        ],
        PromptLanguage.English);

    private static List<SettledDocument> Inspections(int count, int seed = 3)
    {
        var random = new Random(seed);
        var risk = Enumerable.Range(0, 150).Select(_ => random.NextDouble() switch { < 0.75 => 0, < 0.95 => 1, _ => 2 }).ToArray();
        return [.. Enumerable.Range(0, count).Select(i =>
        {
            var place = random.Next(risk.Length);
            return new SettledDocument(
                $"d{i}",
                new Dictionary<string, string> { ["place"] = $"p{place}", ["kind"] = $"k{random.Next(4)}", ["risk"] = Risks[risk[place]] },
                DateTimeOffset.UnixEpoch.AddMinutes(i));
        })];
    }

    private static Dictionary<string, string> Known(string place, string kind) => new() { ["place"] = place, ["kind"] = kind };

    [Fact]
    public void A_field_whose_keys_keep_their_value_gets_a_small_concentration_and_a_field_mixing_values_a_larger_one()
    {
        var chosen = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", Inspections(600));

        chosen.Keys.Should().BeEquivalentTo(["place", "kind"]);
        chosen["place"].Should().BeLessThan(0.05);
        chosen["kind"].Should().BeGreaterThan(chosen["place"] * 10);
    }

    [Fact]
    public void A_key_seen_once_answers_for_its_value_over_a_broad_key_once_concentrations_are_chosen()
    {
        var documents = Inspections(600);
        var memory = new FieldMemory();
        foreach (var document in documents)
        {
            memory.Put(Inspection(), document);
        }

        // A place seen once, with a low risk, visited again under a kind whose keys mostly hold high.
        var seenOnce = new SettledDocument("once", new Dictionary<string, string> { ["place"] = "new-place", ["kind"] = "k0", ["risk"] = "low" }, DateTimeOffset.UnixEpoch.AddDays(30));
        memory.Put(Inspection(), seenOnce);
        var chosen = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", [.. documents, seenOnce]);

        var without = memory.Rank(Inspection(), "risk", Known("new-place", "k0"), 1).Single();
        var with = memory.Rank(Inspection(chosen), "risk", Known("new-place", "k0"), 1).Single();

        without.Value.Should().Be("high");
        without.Evidence.Should().Be("kind: k0");
        with.Value.Should().Be("low");
        with.Evidence.Should().Be("place: new-place");
    }

    [Fact]
    public void Concentrations_of_one_score_exactly_as_none()
    {
        var memory = new FieldMemory();
        foreach (var document in Inspections(300))
        {
            memory.Put(Inspection(), document);
        }

        var ones = new Dictionary<string, double> { ["place"] = 1, ["kind"] = 1 };
        foreach (var (place, kind) in new[] { ("p3", "k1"), ("p40", "k2"), ("p149", "k0") })
        {
            var without = memory.Rank(Inspection(), "risk", Known(place, kind), 3);
            var with = memory.Rank(Inspection(ones), "risk", Known(place, kind), 3);
            with.Should().Equal(without);
        }
    }

    [Fact]
    public void The_same_documents_in_any_order_give_the_same_concentrations()
    {
        var documents = Inspections(400);
        var reversed = Enumerable.Reverse(documents).ToList();

        var forward = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", documents);
        var backward = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", reversed);

        backward.Should().Equal(forward);
    }

    [Fact]
    public void Without_a_key_seen_again_no_concentration_is_chosen()
    {
        var once = new SettledDocument("d0", new Dictionary<string, string> { ["place"] = "p0", ["kind"] = "k0", ["risk"] = "high" }, DateTimeOffset.UnixEpoch);

        ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", [once]).Should().BeEmpty();
    }

    [Fact]
    public void A_field_whose_keys_were_seen_again_once_with_their_value_moves_only_part_of_the_way_from_one()
    {
        // One prediction of the value a key had, beside the two anchoring ones: (0 + 1) / α = 3 / (1 + α), α = 0.5.
        var documents = new[]
        {
            new SettledDocument("d0", new Dictionary<string, string> { ["place"] = "p0", ["risk"] = "high" }, DateTimeOffset.UnixEpoch),
            new SettledDocument("d1", new Dictionary<string, string> { ["place"] = "p0", ["risk"] = "high" }, DateTimeOffset.UnixEpoch.AddMinutes(1)),
        };

        var chosen = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "risk", documents);

        chosen["place"].Should().BeApproximately(0.5, 1e-12);
    }

    [Fact]
    public void A_field_that_takes_several_values_gets_no_concentrations()
    {
        var form = new FormDefinition(
            "tags",
            [new FieldDefinition("place", FieldRole.Observed), new FieldDefinition("tags", FieldRole.Judged) { Multiple = true }],
            PromptLanguage.English);
        var documents = Enumerable.Range(0, 5).Select(i => new SettledDocument(
            $"d{i}", new Dictionary<string, string> { ["place"] = "p0" }, DateTimeOffset.UnixEpoch.AddMinutes(i))
        {
            Sets = new Dictionary<string, IReadOnlyList<string>> { ["tags"] = ["a", "b"] },
        });

        ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), form, "tags", documents).Should().BeEmpty();
    }

    [Theory]
    [InlineData("nowhere", 0.5)]
    [InlineData("place", 0)]
    [InlineData("place", -1)]
    [InlineData("place", double.NaN)]
    [InlineData("place", double.PositiveInfinity)]
    public void A_concentration_for_an_unknown_field_or_not_above_zero_is_refused(string field, double concentration)
    {
        var act = () => Inspection(new Dictionary<string, double> { [field] = concentration });

        act.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void An_observed_field_is_not_chosen_for()
    {
        var act = () => ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(), "place", Inspections(10));

        act.Should().Throw<ArgumentException>();
    }
}
