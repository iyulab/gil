using AwesomeAssertions;
using Gil.Forms;
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

    [Theory]
    [InlineData("a😀b", 2, "a😀")] // a character beyond U+FFFF is one scalar, never cut in half
    [InlineData("a😀b", 1, "a")]
    [InlineData("ab", 5, "ab")]
    public void A_prefix_is_counted_in_scalars(string text, int scalars, string prefix)
    {
        TextNormal.Prefix(text, scalars).Should().Be(prefix);
        new CoarseLevel(scalars).Of(text).Should().Be(prefix);
    }

    [Theory]
    [InlineData("network", "NET", true)]
    [InlineData("İstanbul", "i", true)] // the simple lowercase mapping takes U+0130 to U+0069
    [InlineData("𐐨x", "𐐀", true)] // a scalar beyond U+FFFF is compared as one
    [InlineData("a b", "a b", true)] // typed text is not normalized: spaces count as typed
    [InlineData("ab", "abc", false)]
    [InlineData("😃", "😀", false)] // shared high surrogate, different characters
    public void Typed_text_begins_a_value_scalar_by_scalar_ignoring_case(string value, string typed, bool begins)
    {
        FieldMemory.Begins(value, typed).Should().Be(begins);
    }

    [Fact]
    public void A_character_beyond_the_basic_plane_typed_is_one_character_typed()
    {
        var field = new FieldDefinition("f", FieldRole.Judged) { KeyThreshold = 9, TypedKeyThresholds = [0.1, 0.2] };

        FieldMemory.TypedLength("😀").Should().Be(1);
        field.KeyThresholdFor(FieldMemory.TypedLength("😀")).Should().Be(0.1);
    }

    [Fact]
    public void Values_are_ordered_by_scalar_value_not_by_utf16_code_unit()
    {
        // U+FF21 sorts before U+1D400 by scalar value; by UTF-16 code unit the surrogate U+D835 would come first.
        Settlement.Set(["𝐀", "Ａ"]).Values.Should().Equal("Ａ", "𝐀");
    }
}
