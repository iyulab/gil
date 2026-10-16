using System.Security.Cryptography;
using System.Text;
using AwesomeAssertions;
using Gil.Forms;
using Gil.Memory;
using SupportedSurface;

namespace Gil.Tests.Forms;

/// <summary>
/// The form walkthrough that is compiled with no experimental diagnostic suppressed (tests/SupportedSurface) also runs,
/// over the lexical document memory. A document memory of another kind takes the same slot through the experimental
/// overloads (GIL0004).
/// </summary>
public sealed class SupportedSurfaceTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Fact]
    public async Task The_walkthrough_runs_on_a_lexical_document_memory()
    {
        (await FormsWalkthrough.RunAsync()).Should().BeGreaterThan(3);
    }

    [Fact]
    public async Task An_embedding_memory_behind_a_breaker_takes_the_document_memory_slot_through_the_experimental_overloads()
    {
        var form = new FormDefinition("ticket",
        [
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.5 },
        ], PromptLanguage.English);
        var saved = new[] { ("t/1", "vpn drops"), ("t/2", "vpn drops again"), ("t/3", "invoice wrong") }
            .Select((d, i) => new SettledDocument(d.Item1, new Dictionary<string, string> { ["summary"] = d.Item2, ["team"] = i < 2 ? "network" : "finance" }, DateTimeOffset.UnixEpoch.AddMinutes(i)))
            .ToList();
        IMemory memory = new CircuitBreakingMemory(new EmbeddingMemory(new Hashed()), TimeSpan.FromSeconds(30));

        var forms = new FormResolver(new FieldMemory(), memory);
        await forms.RebuildAsync(form, saved, Ct);
        var team = (await forms.SuggestAsync(form, "t/4", new Dictionary<string, string> { ["summary"] = "vpn drops" }, Ct)).Single();
        var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new EmbeddingMemory(new Hashed()), form, "team", saved, 0.5, 1, cancellationToken: Ct);

        team.Candidates[0].Value.Should().Be("network");
        team.Source.Should().Be(FieldSource.SimilarDocument);
        layers.Memory.Lookups.Should().Be(saved.Count - 1);
    }

    [Fact]
    public async Task An_embedding_memory_built_from_a_model_reports_no_energy()
    {
        var memory = new EmbeddingMemory(new Hashed());

        var remembered = await memory.RememberAsync("task", "k1", "vpn drops", "network", "t1", Ct);
        var (match, energy) = await memory.LookupAsync("task", "vpn drops", "t2", Ct);

        remembered.Should().Be(0);
        energy.Should().Be(0);
        match!.Answer.Should().Be("network");
    }

    private sealed class Hashed : IEmbeddingModel
    {
        public Task<EmbeddingResult> EmbedAsync(IReadOnlyList<string> texts, CancellationToken cancellationToken = default)
        {
            var vectors = texts.Select(t => SHA256.HashData(Encoding.UTF8.GetBytes(t)).Take(8).Select(b => b / 255f - 0.5f).ToArray()).ToList();
            return Task.FromResult(new EmbeddingResult(vectors, "e", texts.Count, 1, "{}"));
        }
    }
}
