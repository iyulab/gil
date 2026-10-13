using System.Security.Cryptography;
using System.Text;
using AwesomeAssertions;
using Gil.Memory;
using SupportedSurface;

namespace Gil.Tests.Forms;

/// <summary>
/// The form walkthrough that is compiled with no experimental diagnostic suppressed (tests/SupportedSurface) also runs —
/// over a lexical document memory and over an embedding memory built straight from an embedding model.
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
    public async Task The_walkthrough_runs_on_an_embedding_memory_built_from_a_model()
    {
        (await FormsWalkthrough.RunAsync(new Hashed())).Should().BeGreaterThan(3);
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
