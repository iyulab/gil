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

/// <summary>The core's time: 100-nanosecond ticks since the Unix epoch, UTC.</summary>
internal static class Time
{
    public static long Ticks(DateTimeOffset time) => time.UtcTicks - DateTimeOffset.UnixEpoch.UtcTicks;

    public static DateTimeOffset Of(long ticks) => new(DateTimeOffset.UnixEpoch.UtcTicks + ticks, TimeSpan.Zero);
}
