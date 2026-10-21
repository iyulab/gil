using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Review.Tests;

/// <summary>The binding reproduces the language-neutral review golden (tests/fixtures/review/golden.jsonl): the same
/// inputs as the Rust crate's golden test, through the public .NET surface, every double compared bit for bit (the
/// golden's null is NaN).</summary>
public sealed class GoldenTests
{
    private static readonly Stratum[] EstimateStrata = [new("A", 400), new("B", 1_500), new("C", 6)];

    private static JsonNode[] Golden() =>
        File.ReadAllLines(Path.Combine(AppContext.BaseDirectory, "review", "golden.jsonl"))
            .Select(line => JsonNode.Parse(line)!)
            .ToArray();

    private static readonly double[] ScoresA = [0.0, 1.0, 1.0, 0.5, 1.0, 0.25, 1.0, 0.0];
    private static readonly double[] ScoresC = [1.0, 0.0, 1.0, 1.0, 0.0, 1.0];

    private static ScoredItem[] PlainItems() =>
    [
        .. ScoresA.Select(s => new ScoredItem(0, s)),
        .. Enumerable.Range(0, 9).Select(_ => new ScoredItem(1, 1.0)),
        .. ScoresC.Select(s => new ScoredItem(2, s)),
    ];

    private static JsonArray Numbers(IEnumerable<double> values) =>
        new([.. values.Select(v => double.IsNaN(v) ? null : JsonValue.Create(v))]);

    private static JsonObject Json(Estimate e) => new()
    {
        ["point"] = e.Point,
        ["se"] = e.StandardError,
        ["normal"] = Numbers([e.Normal.Low, e.Normal.High]),
        ["bootstrap"] = Numbers([e.Bootstrap.Low, e.Bootstrap.High]),
        ["sample_size"] = e.SampleSize,
        ["population"] = e.Population,
        ["effective_n"] = e.EffectiveSampleSize,
        ["seed"] = e.Seed,
        ["caveats"] = (int)e.Caveats,
    };

    private static List<JsonNode> Lines()
    {
        var lines = new List<JsonNode>();

        // The draw: the busy stratum's ids out of order, an empty stratum, one smaller than the request.
        var busy = Enumerable.Range(0, 60).Reverse().Select(i => $"b{i:00}").ToArray();
        PopulationStratum[] population = [new("busy", busy), new("empty", []), new("small", ["s3", "s1", "s2"])];
        foreach (var seed in new ulong[] { 0, 1, 7, 20_260_912 })
        {
            var sample = ReviewSampler.DrawPilot(population, 5, seed);
            lines.Add(new JsonObject
            {
                ["kind"] = "draw",
                ["seed"] = seed,
                ["strata"] = new JsonArray([.. sample.Strata.Select(s => (JsonNode)new JsonObject
                {
                    ["name"] = s.Name,
                    ["population"] = s.Population,
                    ["selected"] = new JsonArray([.. s.Selected.Select(id => (JsonNode)id)]),
                })]),
                ["caveats"] = (int)sample.Caveats,
            });
        }

        // The allocation.
        (StratumSpread[] Strata, long Budget)[] cases =
        [
            ([new(100, 0.5), new(900, 0.3)], 100),
            ([new(3, 0.0), new(8, 0.0), new(5, 0.14)], 12),
            ([new(3, 0.9), new(100, 0.3)], 50),
            ([new(7, 0.31), new(70, 0.05), new(700, 0.2), new(7_000, 0.0)], 333),
        ];
        foreach (var (strata, budget) in cases)
        {
            var counts = ReviewSampler.AllocateNeyman(strata, budget);
            lines.Add(new JsonObject
            {
                ["kind"] = "allocation",
                ["budget"] = budget,
                ["counts"] = new JsonArray([.. counts.Select(c => (JsonNode)c)]),
            });
        }

        // The bound.
        foreach (var n in new long[] { 1, 2, 3, 15, 30, 300, 4_096, 1_000_000 })
        {
            lines.Add(new JsonObject
            {
                ["kind"] = "bound",
                ["n"] = n,
                ["variance"] = ReviewEstimator.UnobservedVariationBound(n),
            });
        }

        // The estimate.
        var plain = PlainItems();
        var weighted = plain.Select((it, i) => it with { Weight = 1.0 + (i % 4 * 2.5) }).ToArray();
        foreach (var (name, items) in new[] { ("plain", plain), ("weighted", weighted) })
        {
            foreach (var (replicates, seed) in new[] { (0, 0UL), (2_000, 7UL) })
            {
                lines.Add(new JsonObject
                {
                    ["kind"] = "estimate",
                    ["items"] = name,
                    ["replicates"] = replicates,
                    ["estimate"] = Json(ReviewEstimator.Estimate(EstimateStrata, items, replicates, seed)),
                });
            }
        }

        var upper = plain.ToArray();
        upper[0] = upper[0] with { Score = 1.0 };
        var (low, high) = ReviewEstimator.EstimateBounds(EstimateStrata, plain, upper, 500, 3);
        lines.Add(new JsonObject { ["kind"] = "bounds", ["lower"] = Json(low), ["upper"] = Json(high) });

        // Agreement over three outcomes.
        (int A, int B)[] pattern = [(0, 0), (0, 0), (1, 1), (0, 1), (2, 2), (1, 0), (0, 0), (2, 0), (1, 1), (0, 0)];
        var verdicts = pattern
            .SelectMany((p, i) => new[] { new Verdict($"i{i}", "c", p.B, "zed"), new Verdict($"i{i}", "c", p.A, "amy") })
            .ToArray();
        var a = ReviewAgreement.Compute(verdicts, 3);
        lines.Add(new JsonObject
        {
            ["kind"] = "agreement",
            ["compared_pairs"] = a.ComparedPairs,
            ["observed"] = a.Observed,
            ["kappa"] = double.IsNaN(a.Kappa) ? null : a.Kappa,
            ["ac1"] = a.Ac1,
            ["specific"] = Numbers(a.Specific),
            ["prevalence_max"] = a.PrevalenceMax,
        });
        return lines;
    }

    /// <summary>Two JSON trees are the same when their shape and strings match and every number is the same double,
    /// bit for bit.</summary>
    private static void SameBits(JsonNode? got, JsonNode? want, string path)
    {
        switch (want)
        {
            case null:
                got.Should().BeNull(path);
                return;
            case JsonObject w:
                var g = got.Should().BeOfType<JsonObject>(path).Subject;
                g.Select(p => p.Key).Should().Equal(w.Select(p => p.Key), path);
                foreach (var (key, value) in w)
                {
                    SameBits(g[key], value, $"{path}.{key}");
                }

                return;
            case JsonArray w:
                var ga = got.Should().BeOfType<JsonArray>(path).Subject;
                ga.Count.Should().Be(w.Count, path);
                for (var i = 0; i < w.Count; i++)
                {
                    SameBits(ga[i], w[i], $"{path}[{i}]");
                }

                return;
            default:
                var wv = want.AsValue();
                got.Should().NotBeNull(path);
                if (wv.TryGetValue<string>(out var s))
                {
                    got!.GetValue<string>().Should().Be(s, path);
                    return;
                }

                var bits = BitConverter.DoubleToInt64Bits(got!.GetValue<double>());
                bits.Should().Be(BitConverter.DoubleToInt64Bits(wv.GetValue<double>()), path);
                return;
        }
    }

    [Fact]
    public void The_binding_reproduces_the_review_golden()
    {
        var want = Golden();
        var got = Lines();
        got.Should().HaveCount(want.Length);
        for (var i = 0; i < want.Length; i++)
        {
            // Through text, as a consumer would read the values back.
            SameBits(JsonNode.Parse(got[i].ToJsonString()), want[i], $"line {i + 1}");
        }
    }
}
