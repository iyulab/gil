using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using AwesomeAssertions;
using Gil.Memory;

namespace Gil.Tests.Memory;

/// <summary>
/// The text golden: for texts that exercise each step of docs/text-normalization.md, the normalized text, the key it
/// makes and its n-grams with their counts in scalar order, as JSON lines another implementation can reproduce byte for
/// byte (tests/fixtures/core/text.jsonl). Set GIL_WRITE_CORE_GOLDEN to a directory to rewrite the file.
/// </summary>
public sealed class TextGoldenTests
{
    private const int MaxKeyLength = 60; // FieldMemory's default
    private const int MinGram = 2; // LexicalMemory's defaults
    private const int MaxGram = 3;

    private static readonly JsonSerializerOptions Json = new() { Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping };

    // Each line names what it exercises; the texts are the golden's input.
    private static readonly string[] Texts =
    [
        "Hello  World", // a run of spaces is one
        "  leading and trailing\t\n", // no space at either end, tab and newline are whitespace
        "\u00A0nbsp\u2028line\u3000ideographic\u205Fmath\u0085next", // other White_Space code points
        "\uFB01le", // NFKC: a ligature becomes two letters
        "\uFF21\uFF22\uFF23\u3000\uFF44\uFF45\uFF46", // NFKC: full-width forms
        "\u2460\u2461 \u216B", // NFKC: circled digits, a roman numeral
        "\u212A\u00C5", // NFKC: Kelvin sign and a precomposed letter
        "e\u0301cole", // NFKC composes a combining accent
        "\u0130stanbul", // U+0130 lowers to U+0069
        "\u03A3\u0391\u03A3 \u03A3", // capital sigma is always U+03C3
        "Stra\u00DFe \u1E9E", // sharp s, capital sharp s
        "\u01C4", // NFKC splits the titlecase digraph's compatibility form, then each lowers
        "\U0001D538\U0001D539", // mathematical letters beyond U+FFFF become ASCII
        "a\U0001F600b", // a character beyond U+FFFF is one scalar
        "\uE000x\U0001F600y", // scalar order differs from UTF-16 order here
        "\uD55C\uAE00 \uC785\uB825", // Hangul
        "\uAC00", // one syllable
        "x", // shorter than the longest n-gram once padded
        "", // nothing
        "   ", // whitespace only
        new string('a', 60), // as long as a key may be
        new string('b', 61), // too long to be a key
        "A\u0300\u0301\u0302", // several combining marks
    ];

    [Fact]
    public async Task The_text_normalization_reproduces_its_golden()
    {
        var path = Path.Combine(AppContext.BaseDirectory, "core", "text.jsonl");
        if (Environment.GetEnvironmentVariable("GIL_WRITE_CORE_GOLDEN") is { } target)
        {
            Directory.CreateDirectory(target);
            await File.WriteAllLinesAsync(Path.Combine(target, "text.jsonl"), Texts.Select(Line), TestContext.Current.CancellationToken);
            return;
        }

        var expected = await File.ReadAllLinesAsync(path, TestContext.Current.CancellationToken);
        var lines = expected.Select(l => Line(JsonNode.Parse(l)!["text"]!.GetValue<string>()));

        lines.Should().Equal(expected);
    }

    private static string Line(string text)
    {
        var normal = TextNormal.Collapse(text);
        var key = normal.Length > 0 && TextNormal.Scalars(normal) <= MaxKeyLength ? normal : null;
        return new JsonObject
        {
            ["text"] = text,
            ["normal"] = normal,
            ["key"] = key,
            ["grams"] = new JsonArray([.. LexicalMemory.Grams(text, MinGram, MaxGram).Select(g => (JsonNode)new JsonArray(g.Gram, g.Count))]),
        }.ToJsonString(Json);
    }
}
