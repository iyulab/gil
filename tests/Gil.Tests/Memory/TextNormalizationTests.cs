using AwesomeAssertions;
using Gil.Memory;

namespace Gil.Tests.Memory;

/// <summary>The normalisation and orders docs/text-normalization.md specifies, case by case.</summary>
public sealed class TextNormalizationTests
{
    private static CancellationToken Ct => TestContext.Current.CancellationToken;

    [Theory]
    [InlineData("ＡＢＣ", "abc")] // NFKC folds the full-width forms
    [InlineData("ΟΔΟΣ", "οδοσ")] // simple mapping: a capital sigma is always σ, never the final ς
    [InlineData("İstanbul", "istanbul")] // simple mapping: U+0130 is one i, not i with a combining dot
    [InlineData("  a\u00A0\u3000b\u2028c  ", "a b c")] // every White_Space run is one space, none at the ends
    [InlineData("𐐀x", "𐐨x")] // a scalar beyond U+FFFF is lower-cased as one
    public void Text_is_normalized_scalar_by_scalar(string text, string normal)
    {
        TextNormal.Collapse(text).Should().Be(normal);
    }

    [Fact]
    public async Task N_grams_are_runs_of_scalars_so_characters_beyond_the_basic_plane_do_not_share_halves()
    {
        // 😀 and 😃 share their high surrogate; cut into UTF-16 code units, their n-grams would overlap.
        var memory = new LexicalMemory(minGram: 2, maxGram: 2);
        await memory.RememberAsync("task", "a", "😀", "smile", "t1", Ct);

        var (match, _) = await memory.LookupAsync("task", "😃", "t2", Ct);

        match!.Similarity.Should().Be(0);
    }

    [Fact]
    public void Values_are_ordered_by_scalar_value_not_by_utf16_code_unit()
    {
        // U+FF21 sorts before U+1D400 by scalar value; by UTF-16 code unit the surrogate U+D835 would come first.
        Settlement.Set(["𝐀", "Ａ"]).Values.Should().Equal("Ａ", "𝐀");
    }
}
