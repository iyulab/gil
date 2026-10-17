using AwesomeAssertions;
using Gil.Forms;

namespace Gil.Tests.Forms;

/// <summary>
/// Choosing the fields that key a judged field in the settled field memory by replaying its key layer: where a few fields
/// decide a field with many values and the others only blur it, keying it by those few answers more often at the same
/// precision — while the similar document layer and a model keep every field the field rests on.
/// </summary>
public sealed class KeyFieldsSelectionTests
{
    private static readonly string[] Parts = ["valve", "pump", "seal", "hose", "gauge", "filter", "fan", "relay"];
    private static readonly string[] Places = ["left", "right", "aft"];

    // The code follows from the part. The model is drawn from many values, each seen a few times: a key seen rarely lends
    // its few codes most of their strength, so it pushes wrong codes up. The place and the shift are drawn from a few.
    private static FormDefinition Report(IReadOnlyList<string>? keyFields = null, IReadOnlyList<string>? dependsOn = null) => new(
        "report",
        [
            new FieldDefinition("part", FieldRole.Observed),
            new FieldDefinition("place", FieldRole.Observed),
            new FieldDefinition("model", FieldRole.Observed),
            new FieldDefinition("shift", FieldRole.Observed),
            new FieldDefinition("code", FieldRole.Judged) { DependsOn = dependsOn, KeyFields = keyFields },
        ],
        PromptLanguage.English);

    private static List<SettledDocument> Reports(int count, int seed = 5)
    {
        var random = new Random(seed);
        var documents = new List<SettledDocument>();
        for (var i = 0; i < count; i++)
        {
            var part = random.Next(Parts.Length);
            var place = random.Next(Places.Length);
            // A tenth of the codes are miscoded at random, so no key is perfectly pure.
            var code = random.NextDouble() < 0.1 ? random.Next(Parts.Length) : part;
            documents.Add(new SettledDocument(
                $"d{i}",
                new Dictionary<string, string>
                {
                    ["part"] = Parts[part],
                    ["place"] = Places[place],
                    ["model"] = $"m{random.Next(300)}",
                    ["shift"] = $"s{random.Next(3)}",
                    ["code"] = $"c{code}",
                },
                DateTimeOffset.UnixEpoch.AddMinutes(i)));
        }

        return documents;
    }

    [Fact]
    public void The_fields_that_decide_the_field_are_chosen_and_answer_more_often_than_every_field()
    {
        var documents = Reports(1500);

        var choice = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "code", documents, 0.8, 30);

        choice.KeyFields.Should().NotBeNull();
        choice.KeyFields![0].Should().Be("part");
        choice.KeyFields.Should().NotContain("model");
        choice.Chosen.Chosen!.Answered.Should().BeGreaterThan(choice.AllFields.Chosen?.Answered ?? 0);
        choice.Chosen.Chosen.Precision.Should().BeGreaterThanOrEqualTo(0.8);
        // Every field it rests on alone, in the form's order, then the additions tried.
        choice.Trials.Take(4).Select(t => t.KeyFields.Single()).Should().Equal("part", "place", "model", "shift");
    }

    [Fact]
    public void The_choice_is_the_replay_SelectKeyThreshold_makes_with_those_fields()
    {
        var documents = Reports(1500);
        var choice = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "code", documents, 0.8, 30);

        var again = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Report(choice.KeyFields), "code", documents, 0.8, 30);
        var all = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Report(), "code", documents, 0.8, 30);

        choice.Chosen.Should().BeEquivalentTo(again);
        choice.AllFields.Should().BeEquivalentTo(all);
    }

    [Fact]
    public void Keying_by_a_few_fields_replays_the_key_layer_as_resting_on_them_did()
    {
        // KeyFields narrows the key layer exactly as DependsOn did — the same keys, the same replay.
        var documents = Reports(1500);

        var keyed = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Report(keyFields: ["part", "place"]), "code", documents, 0.8, 30);
        var resting = ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Report(dependsOn: ["part", "place"]), "code", documents, 0.8, 30);

        keyed.Should().BeEquivalentTo(resting);
    }

    [Fact]
    public void Key_fields_narrow_only_the_key_layer_and_the_lookup_still_reads_every_field_the_field_rests_on()
    {
        var form = Report(keyFields: ["part"]);
        var values = new Dictionary<string, string> { ["part"] = "valve", ["place"] = "aft", ["model"] = "m7", ["shift"] = "s1" };

        form.Keys("part", "code").Should().BeTrue();
        form.Keys("model", "code").Should().BeFalse();
        form.Supports("model", "code").Should().BeTrue();
        FormResolver.EvidenceValues(form, "code", values).Select(v => v.Key).Should().Equal("part", "place", "model", "shift");
    }

    [Fact]
    public void The_field_s_own_KeyFields_is_ignored_and_its_DependsOn_bounds_the_fields_tried()
    {
        var documents = Reports(1500);

        var unset = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "code", documents, 0.8, 30);
        var set = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(keyFields: ["model"]), "code", documents, 0.8, 30);
        var bounded = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(dependsOn: ["part", "model"]), "code", documents, 0.8, 30);

        set.KeyFields.Should().Equal(unset.KeyFields!);
        set.AllFields.Should().BeEquivalentTo(unset.AllFields);
        bounded.Trials.SelectMany(t => t.KeyFields).Distinct().Should().BeSubsetOf(["part", "model"]);
        bounded.AllFields.Should().BeEquivalentTo(ThresholdSelection.SelectKeyThreshold(new FieldMemory(), Report(dependsOn: ["part", "model"]), "code", documents, 0.8, 30));
    }

    [Theory]
    [InlineData("code")] // the field itself
    [InlineData("missing")] // not a field of the form
    [InlineData("place")] // outside the fields it rests on
    public void Key_fields_must_be_fields_the_field_rests_on(string key)
    {
        var act = () => Report(keyFields: [key], dependsOn: ["part", "model"]);

        act.Should().Throw<ArgumentException>().WithMessage($"*keyed by '{key}'*");
    }

    [Fact]
    public void A_field_that_is_not_evidence_cannot_key_another()
    {
        var act = () => new FormDefinition(
            "report",
            [
                new FieldDefinition("part", FieldRole.Observed),
                new FieldDefinition("reporter", FieldRole.Observed) { UseAsEvidence = false },
                new FieldDefinition("code", FieldRole.Judged) { KeyFields = ["reporter"] },
            ],
            PromptLanguage.English);

        act.Should().Throw<ArgumentException>().WithMessage("*keyed by 'reporter'*");
    }

    [Fact]
    public void A_field_naming_what_the_field_describes_is_kept_though_a_broad_field_alone_answers_more_often()
    {
        // Each outlet keeps its own risk, and the kind of visit mostly follows it; an inspector says nothing. The kind
        // alone answers about every visit, the outlet only the outlets that come again — but there rightly, where the kind
        // alone is wrong. Answers alone would key the risk by the kind; the gain over the target keeps the outlet.
        var random = new Random(3);
        string[] kinds = ["routine", "complaint", "license"];
        var risks = Enumerable.Range(0, 300).Select(_ => random.Next(3)).ToArray();
        var documents = new List<SettledDocument>();
        for (var i = 0; i < 800; i++)
        {
            var outlet = random.Next(risks.Length);
            var kind = random.NextDouble() < 0.85 ? kinds[risks[outlet]] : kinds[random.Next(kinds.Length)];
            documents.Add(new SettledDocument(
                $"d{i}",
                new Dictionary<string, string> { ["outlet"] = $"o{outlet}", ["kind"] = kind, ["inspector"] = $"i{random.Next(40)}", ["risk"] = $"r{risks[outlet]}" },
                DateTimeOffset.UnixEpoch.AddMinutes(i)));
        }

        FormDefinition Inspection(IReadOnlyDictionary<string, double>? concentrations) => new(
            "inspection",
            [
                new FieldDefinition("outlet", FieldRole.Observed),
                new FieldDefinition("kind", FieldRole.Observed),
                new FieldDefinition("inspector", FieldRole.Observed),
                new FieldDefinition("risk", FieldRole.Judged) { KeyConcentrations = concentrations },
            ],
            PromptLanguage.English);
        var form = Inspection(ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), Inspection(null), "risk", documents));
        static double Gain(ThresholdChoice c) => (c.Precision - 0.8) * c.AnsweredWeight;

        var choice = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "risk", documents, 0.8, 10);

        choice.KeyFields.Should().BeEquivalentTo(["outlet", "kind"]);
        var kindAlone = choice.Trials.Single(t => t.KeyFields.SequenceEqual(["kind"])).Replay.Chosen!;
        kindAlone.Answered.Should().BeGreaterThanOrEqualTo(choice.Chosen.Chosen!.Answered); // an addition answering no more stops answers alone
        kindAlone.Answered.Should().BeGreaterThan(choice.AllFields.Chosen!.Answered);
        Gain(choice.Chosen.Chosen).Should().BeGreaterThan(Gain(kindAlone));
        Gain(choice.Chosen.Chosen).Should().BeGreaterThan(Gain(choice.AllFields.Chosen));
    }

    [Fact]
    public void Where_no_narrower_set_gains_more_KeyFields_stays_unset()
    {
        // One field decides the code on its own and the other carries the same information: narrowing gains nothing.
        var form = new FormDefinition(
            "pair",
            [
                new FieldDefinition("part", FieldRole.Observed),
                new FieldDefinition("alias", FieldRole.Observed),
                new FieldDefinition("code", FieldRole.Judged),
            ],
            PromptLanguage.English);
        var documents = Enumerable.Range(0, 400).Select(i => new SettledDocument(
            $"d{i}",
            new Dictionary<string, string> { ["part"] = Parts[i % Parts.Length], ["alias"] = $"a{i % Parts.Length}", ["code"] = $"c{i % Parts.Length}" },
            DateTimeOffset.UnixEpoch.AddMinutes(i))).ToList();

        var choice = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "code", documents, 0.8, 30);

        choice.KeyFields.Should().BeNull();
        choice.Chosen.Should().Be(choice.AllFields);
    }

    [Fact]
    public void A_field_that_is_not_evidence_is_never_tried()
    {
        var form = new FormDefinition(
            "report",
            [
                new FieldDefinition("part", FieldRole.Observed),
                new FieldDefinition("reporter", FieldRole.Observed) { UseAsEvidence = false },
                new FieldDefinition("code", FieldRole.Judged),
            ],
            PromptLanguage.English);
        var documents = Reports(200).Select(d => d with
        {
            Values = new Dictionary<string, string> { ["part"] = d.Values["part"], ["reporter"] = "kim", ["code"] = d.Values["code"] },
        }).ToList();

        var choice = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "code", documents, 0.8, 10);

        choice.Trials.SelectMany(t => t.KeyFields).Should().NotContain("reporter");
    }

    [Fact]
    public void Only_a_judged_field_can_be_asked_about()
    {
        var act = () => ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "part", Reports(10), 0.8, 5);

        act.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void Counting_typing_brings_the_typed_thresholds_for_the_choice()
    {
        var documents = Reports(1500);

        var untyped = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "code", documents, 0.8, 30);
        var typed = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), Report(), "code", documents, 0.8, 30, typedLongest: 2);

        untyped.Typed.Should().BeNull();
        untyped.Trials.Should().OnlyContain(t => t.Typed == null);
        typed.Typed.Should().NotBeNull();
        typed.Trials.Should().OnlyContain(t => t.Typed != null && t.Typed.ByLength.Count == 2);

        // The typed thresholds that come with the choice are those chosen on the form keyed by it, with its key threshold.
        var keyed = Report(typed.KeyFields);
        keyed = new FormDefinition(keyed.Name, [.. keyed.Fields.Select(f => f.Name == "code" ? f with { KeyThreshold = typed.Chosen.Chosen?.Threshold } : f)], keyed.Language);
        var again = ThresholdSelection.SelectTypedKeyThresholds(new FieldMemory(), keyed, "code", documents, 0.8, 30, 2);
        typed.Typed!.Thresholds.Should().Equal(again.Thresholds);
    }

    [Fact]
    public void Typing_is_not_counted_for_a_field_that_takes_several_values()
    {
        var form = new FormDefinition("paper",
            [new FieldDefinition("venue", FieldRole.Observed), new FieldDefinition("topics", FieldRole.Judged) { Multiple = true }], PromptLanguage.English);

        var select = () => ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "topics", [], 0.8, 5, typedLongest: 1);

        select.Should().Throw<ArgumentException>().WithMessage("*'topics' takes several values*");
    }
}
