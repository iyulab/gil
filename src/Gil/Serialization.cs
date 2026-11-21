using System.Text.Json;
using System.Text.Json.Serialization;

namespace Gil;

/// <summary>A schema in the core's form.</summary>
internal sealed record SchemaForm(string Name, IReadOnlyList<AttributeDefinition> Attributes);

/// <summary>The core's JSON form of schemas and records, written and read without reflection: snake_case names, settings
/// left out where they are null, and time in 100-nanosecond ticks since the Unix epoch, UTC.</summary>
[JsonSourceGenerationOptions(
    PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower,
    DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    Converters = [typeof(TicksConverter)])]
[JsonSerializable(typeof(SchemaForm))]
[JsonSerializable(typeof(Record))]
[JsonSerializable(typeof(IReadOnlyList<Answer>))]
[JsonSerializable(typeof(Answer))]
[JsonSerializable(typeof(KeyEvidence))]
[JsonSerializable(typeof(RecordEvidence))]
[JsonSerializable(typeof(LayerSelection))]
[JsonSerializable(typeof(ThresholdReplay))]
[JsonSerializable(typeof(IReadOnlyList<ScoredDecision>))]
[JsonSerializable(typeof(KeyFieldsChoice))]
[JsonSerializable(typeof(IReadOnlyDictionary<string, double>))]
[JsonSerializable(typeof(LayerThresholds))]
[JsonSerializable(typeof(Tuning))]
[JsonSerializable(typeof(KeeperChoice))]
[JsonSerializable(typeof(Stale))]
[JsonSerializable(typeof(UnseenMass))]
[JsonSerializable(typeof(IReadOnlyList<ElementAnswer>))]
[JsonSerializable(typeof(IReadOnlyDictionary<string, AttributeUsage>))]
[JsonSerializable(typeof(IReadOnlyList<AskAdvice>))]
internal sealed partial class CoreJson : JsonSerializerContext
{
}

/// <summary>A time as the core keeps it: 100-nanosecond ticks since the Unix epoch, UTC.</summary>
internal sealed class TicksConverter : JsonConverter<DateTimeOffset>
{
    public override DateTimeOffset Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        Time.Of(reader.GetInt64());

    public override void Write(Utf8JsonWriter writer, DateTimeOffset value, JsonSerializerOptions options) =>
        writer.WriteNumberValue(Time.Ticks(value));
}

/// <summary>A fingerprint as the core writes it: 16 lowercase hexadecimal digits, a text — a number above 2^53 does not
/// survive a JSON reader that holds numbers as doubles, and a fingerprint is only compared for equality. A fingerprint
/// written as a number (before they were text) still reads.</summary>
internal sealed class FingerprintConverter : JsonConverter<ulong>
{
    public override ulong Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.Number)
        {
            return reader.GetUInt64();
        }

        if (reader.TokenType == JsonTokenType.String)
        {
            var text = reader.GetString()!;
            if (text.Length is >= 1 and <= 16 && text.All(Uri.IsHexDigit) &&
                ulong.TryParse(text, System.Globalization.NumberStyles.AllowHexSpecifier,
                    System.Globalization.CultureInfo.InvariantCulture, out var value))
            {
                return value;
            }
        }

        throw new JsonException("a fingerprint is up to 16 hexadecimal digits, or a non-negative integer");
    }

    public override void Write(Utf8JsonWriter writer, ulong value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.ToString("x16", System.Globalization.CultureInfo.InvariantCulture));
}

/// <summary>The core's time: 100-nanosecond ticks since the Unix epoch, UTC.</summary>
internal static class Time
{
    public static long Ticks(DateTimeOffset time) => time.UtcTicks - DateTimeOffset.UnixEpoch.UtcTicks;

    public static DateTimeOffset Of(long ticks) => new(DateTimeOffset.UnixEpoch.UtcTicks + ticks, TimeSpan.Zero);
}
