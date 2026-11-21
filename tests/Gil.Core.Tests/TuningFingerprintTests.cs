using System.Text.Json.Nodes;
using AwesomeAssertions;

namespace Gil.Core.Tests;

/// <summary>A tuning's fingerprints are 16 hexadecimal digits in its JSON, so a JSON reader that holds numbers as
/// doubles cannot round them; one written as a number still reads.</summary>
public sealed class TuningFingerprintTests
{
    // Both above 2^53: a double would round them.
    private const ulong SchemaValue = 17294897944762421986UL;
    private const string SchemaText = "f003d20c683292e2";
    private const ulong ChoiceValue = 3239956312049003278UL;
    private const string ChoiceText = "2cf6a3198b772b0e";

    private static string Json(string schema, string choice) =>
        $$"""{"version":"v","scale":1,"schema":{{schema}},"choice":{{choice}},"settled":10,"target_precision":0.8,"attributes":[]}""";

    [Fact]
    public void The_fingerprints_are_written_as_sixteen_hexadecimal_digits()
    {
        var tuning = Tuning.FromJson(Json("17294897944762421986", "3239956312049003278"));

        tuning.SchemaFingerprint.Should().Be(SchemaValue);
        tuning.Choice.Should().Be(ChoiceValue);
        var written = JsonNode.Parse(tuning.ToJson())!.AsObject();
        ((string)written["schema"]!).Should().Be(SchemaText);
        ((string)written["choice"]!).Should().Be(ChoiceText);
        ((string)written["schema"]!).Should().HaveLength(16);
        Tuning.FromJson(tuning.ToJson()).Should().BeEquivalentTo(tuning);
    }

    [Fact]
    public void A_fingerprint_written_as_a_number_or_as_text_reads_the_same()
    {
        var asText = Tuning.FromJson(Json($"\"{SchemaText}\"", $"\"{ChoiceText}\""));
        var asNumber = Tuning.FromJson(Json("17294897944762421986", "3239956312049003278"));
        asText.Should().BeEquivalentTo(asNumber);
        Tuning.FromJson(Json("5", "0")).Choice.Should().Be(0UL, "a tuning that recorded no choice reads as 0");
        Tuning.FromJson(Json("\"5\"", "\"0\"")).SchemaFingerprint.Should().Be(5UL);
    }

    [Theory]
    [InlineData("\"\"")]
    [InlineData("\"xyz\"")]
    [InlineData("\"+1\"")]
    [InlineData("\"0x10\"")]
    [InlineData("\"10000000000000000\"")]
    [InlineData("-1")]
    [InlineData("1.5")]
    [InlineData("true")]
    public void Anything_that_is_not_a_fingerprint_does_not_read(string bad)
    {
        FluentActions.Invoking(() => Tuning.FromJson(Json(bad, "0")))
            .Should().Throw<ArgumentException>();
    }
}
