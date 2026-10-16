using System.Text;

namespace Gil.Memory;

/// <summary>
/// The one text normalisation every memory that compares text by its characters uses, specified scalar by scalar so
/// that another implementation reaches the same text (docs/text-normalization.md): NFKC, then each Unicode scalar
/// value lower-cased by its simple mapping, every run of White_Space one space, no space at either end.
/// </summary>
internal static class TextNormal
{
    /// <summary>NFKC, lower case, every run of whitespace one space, no space at either end.</summary>
    public static string Collapse(string text)
    {
        var normal = new StringBuilder(text.Length);
        Span<char> units = stackalloc char[2];
        var space = true;
        foreach (var rune in text.Normalize(NormalizationForm.FormKC).EnumerateRunes())
        {
            if (Rune.IsWhiteSpace(rune))
            {
                space = true;
                continue;
            }

            if (space && normal.Length > 0)
            {
                normal.Append(' ');
            }

            normal.Append(units[..Lower(rune).EncodeToUtf16(units)]);
            space = false;
        }

        return normal.ToString();
    }

    /// <summary>How many Unicode scalar values <paramref name="text"/> holds.</summary>
    public static int Scalars(string text)
    {
        var count = 0;
        foreach (var _ in text.EnumerateRunes())
        {
            count++;
        }

        return count;
    }

    /// <summary>
    /// The simple lowercase mapping of one scalar. .NET's invariant casing follows it except for U+0130, which it leaves
    /// alone so that casing round-trips; the mapping takes it to U+0069.
    /// </summary>
    private static Rune Lower(Rune rune) => rune.Value == 0x130 ? new Rune('i') : Rune.ToLowerInvariant(rune);

    /// <summary>
    /// Where each Unicode scalar value of <paramref name="text"/> starts, in UTF-16 code units, followed by the text's
    /// length — so scalar <c>i</c> is <c>text[starts[i]..starts[i + 1]]</c>.
    /// </summary>
    public static int[] ScalarStarts(string text)
    {
        var starts = new List<int>(text.Length + 1);
        var at = 0;
        foreach (var rune in text.EnumerateRunes())
        {
            starts.Add(at);
            at += rune.Utf16SequenceLength;
        }

        starts.Add(text.Length);
        return [.. starts];
    }
}
