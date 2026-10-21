namespace Gil.Memory;

/// <summary>
/// Orders strings by Unicode scalar value — the order their UTF-8 or UTF-32 encodings sort in, so any implementation
/// reaches it without UTF-16. It differs from ordinal UTF-16 order only where a character beyond U+FFFF meets one from
/// U+E000 to U+FFFF: a surrogate code unit sorts after every other.
/// </summary>
internal sealed class ScalarOrdinal : IComparer<string>
{
    public static readonly ScalarOrdinal Instance = new();

    public int Compare(string? x, string? y)
    {
        if (ReferenceEquals(x, y))
        {
            return 0;
        }

        if (x is null || y is null)
        {
            return x is null ? -1 : 1;
        }

        var length = Math.Min(x.Length, y.Length);
        for (var i = 0; i < length; i++)
        {
            if (x[i] != y[i])
            {
                return Rank(x[i]) - Rank(y[i]);
            }
        }

        return x.Length - y.Length;
    }

    /// <summary>The code unit moved so that surrogates (U+D800 to U+DFFF) rank above U+E000 to U+FFFF.</summary>
    private static int Rank(char c) => c < 0xD800 ? c : c < 0xE000 ? c + 0x2000 : c - 0x800;
}
