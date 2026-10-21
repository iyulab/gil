using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Gil.Native;

/// <summary>The settings the replays chose for a schema's decided attributes and what they were chosen on — one record a
/// host keeps between runs (<see cref="ToJson"/>, <see cref="FromJson"/>), puts back at start-up (<see cref="Apply"/>)
/// and asks whether it still stands (<see cref="Stale"/>).</summary>
/// <param name="Version">The core's version that chose them.</param>
/// <param name="SchemaFingerprint">The fingerprint of the settings the choice rests on.</param>
/// <param name="Settled">How many records were settled when they were chosen.</param>
/// <param name="TargetPrecision">The precision the replays were asked to meet.</param>
/// <param name="Attributes">Each decided attribute's chosen settings.</param>
public sealed record Tuning(
    string Version,
    [property: JsonPropertyName("schema")] ulong SchemaFingerprint,
    int Settled,
    double TargetPrecision,
    IReadOnlyList<AttributeTuning> Attributes)
{
    /// <summary>The chosen settings as <paramref name="schema"/>'s decided attributes now hold them, chosen on
    /// <paramref name="settled"/> records for <paramref name="targetPrecision"/>; the promises are left unrecorded —
    /// <see cref="Promise"/> records a layers selection's.</summary>
    public static Tuning Capture(Schema schema, int settled, double targetPrecision) =>
        Read(TuningJson.CaptureJson(schema, settled, targetPrecision));

    /// <summary>A tuning kept as JSON in the core's form.</summary>
    /// <exception cref="ArgumentException">The JSON does not read as a tuning.</exception>
    public static Tuning FromJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        try
        {
            return Read(json);
        }
        catch (JsonException e)
        {
            throw new ArgumentException($"a tuning does not read: {e.Message}", nameof(json), e);
        }
    }

    /// <summary>The tuning as JSON in the core's form, to keep.</summary>
    public string ToJson() => JsonSerializer.Serialize(this, CoreJson.Default.Tuning);

    /// <summary>The tuning with what <paramref name="attribute"/>'s layers selection promised: the precisions its chosen
    /// key and memory thresholds promised for the next answers.</summary>
    /// <exception cref="ArgumentException">The tuning has no such attribute.</exception>
    public Tuning Promise(string attribute, LayerThresholds thresholds)
    {
        ArgumentNullException.ThrowIfNull(thresholds);
        return Read(TuningJson.PromiseJson(
            ToJson(), attribute, JsonSerializer.Serialize(thresholds, CoreJson.Default.LayerThresholds)));
    }

    /// <summary>A new schema: <paramref name="schema"/> with these settings put back on its attributes.</summary>
    public Schema Apply(Schema schema) => TuningJson.Apply(ToJson(), schema);

    /// <summary>Why the tuning no longer stands for <paramref name="schema"/> with <paramref name="settled"/> records —
    /// another version of the core, a changed setting it rests on, or a tenth more or fewer records — or null while it
    /// does.</summary>
    public Stale? Stale(Schema schema, int settled) =>
        TuningJson.StaleJson(ToJson(), schema, settled) is { } found
            ? JsonSerializer.Deserialize(found, CoreJson.Default.Stale)
            : null;

    private static Tuning Read(string json) => JsonSerializer.Deserialize(json, CoreJson.Default.Tuning)!;
}

/// <summary>One decided attribute's chosen settings, with the promises the replays made for them.</summary>
/// <param name="Attribute">The decided attribute.</param>
public sealed record AttributeTuning(string Attribute)
{
    /// <summary>Its key attributes.</summary>
    public IReadOnlyList<string>? KeyFields { get; init; }

    /// <summary>How readily each key attribute's keys are believed.</summary>
    public IReadOnlyDictionary<string, double> KeyConcentrations { get; init; } = ReadOnlyDictionary<string, double>.Empty;

    /// <summary>The key layer's threshold.</summary>
    public double? KeyThreshold { get; init; }

    /// <summary>The key layer's thresholds for one, two, … characters typed.</summary>
    public IReadOnlyList<double?>? TypedKeyThresholds { get; init; }

    /// <summary>The similar record layer's threshold.</summary>
    public double? MemoryThreshold { get; init; }

    /// <summary>The similar record layer's similarity floor.</summary>
    public double? MemoryFloor { get; init; }

    /// <summary>The coarse level's key threshold.</summary>
    public double? CoarseKeyThreshold { get; init; }

    /// <summary>The coarse level's memory threshold.</summary>
    public double? CoarseMemoryThreshold { get; init; }

    /// <summary>The coarse level's similarity floor.</summary>
    public double? CoarseMemoryFloor { get; init; }

    /// <summary>The precision the key layer's chosen threshold promised for the next answers; null when none was
    /// recorded.</summary>
    public double? KeyPromise { get; init; }

    /// <summary>The precision the similar record layer's chosen threshold promised for the next answers.</summary>
    public double? MemoryPromise { get; init; }
}

/// <summary>Why a tuning no longer stands.</summary>
[JsonConverter(typeof(StaleConverter))]
public abstract record Stale
{
    private protected Stale()
    {
    }

    /// <summary>Chosen by another version of the core.</summary>
    /// <param name="Chosen">The version that chose it.</param>
    public sealed record Version(string Chosen) : Stale;

    /// <summary>A setting the choice rests on changed.</summary>
    public sealed record Schema : Stale;

    /// <summary>The settled records grew or shrank by a tenth or more since the choice.</summary>
    /// <param name="Chosen">How many were settled when it was chosen.</param>
    /// <param name="Now">How many are settled now.</param>
    public sealed record Settled(int Chosen, int Now) : Stale;
}

/// <summary>One attribute's counts of how its answers fared.</summary>
/// <param name="Decisions">Decisions asked about.</param>
/// <param name="Answered">Of them, answered (a trusted answer shown).</param>
/// <param name="Accepted">Answered and accepted as shown.</param>
/// <param name="Corrected">Answered and settled as another value.</param>
/// <param name="Rejected">Answered and turned down with no value settled.</param>
/// <param name="Filled">Answered and settled by the host on its own.</param>
/// <param name="Checked">Answered and checked by someone who did not see the answer.</param>
/// <param name="CheckedRight">Of the checks, those that agreed with the answer.</param>
/// <param name="Entered">Not answered, and entered by a person.</param>
/// <param name="EnteredOverAnswer">Answered, yet settled as entered with no answer in view — a host that reports its own
/// answers' acceptance as entry; left out of every other count, it should stay 0.</param>
/// <param name="AcceptedOther">Settled by a person who kept a suggestion that was not this decider's — left out of the
/// precision.</param>
/// <param name="CorrectedOther">Settled by a person as another value than a suggestion that was not this decider's — left
/// out of the precision.</param>
/// <param name="Promised">Answered with a promise.</param>
/// <param name="PromiseSum">The sum of those promises.</param>
/// <param name="RightWeight">The weighted count of right answers among those whose rightness is known, a check weighing
/// one ÷ its probability.</param>
/// <param name="JudgedWeight">The weighted count of answers whose rightness is known.</param>
public sealed record AttributeUsage(
    int Decisions,
    int Answered,
    int Accepted,
    int Corrected,
    int Rejected,
    int Filled,
    int Checked,
    int CheckedRight,
    int Entered,
    int EnteredOverAnswer,
    int AcceptedOther,
    int CorrectedOther,
    int Promised,
    double PromiseSum,
    double RightWeight,
    double JudgedWeight)
{
    /// <summary>The decisions not answered.</summary>
    [JsonIgnore]
    public int Abstained => Decisions - Answered;

    /// <summary>The share of decisions answered; null with no decisions.</summary>
    [JsonIgnore]
    public double? AnswerRate => Decisions > 0 ? (double)Answered / Decisions : null;

    /// <summary>The precision the answers realized, among those whose rightness is known; null when none is.</summary>
    [JsonIgnore]
    public double? Realized => JudgedWeight > 0 ? RightWeight / JudgedWeight : null;

    /// <summary>The mean promise of the answers made under one; null when none was.</summary>
    [JsonIgnore]
    public double? Promise => Promised > 0 ? PromiseSum / Promised : null;
}

/// <summary>A reason as the core writes it: <c>{"version": {"chosen"}}</c>, <c>"schema"</c> or <c>{"settled":
/// {"chosen", "now"}}</c>.</summary>
internal sealed class StaleConverter : JsonConverter<Stale>
{
    public override Stale Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.String)
        {
            return reader.GetString() == "schema"
                ? new Stale.Schema()
                : throw new JsonException($"a stale reason is version, schema or settled, not {reader.GetString()}");
        }

        using var document = JsonDocument.ParseValue(ref reader);
        var reason = document.RootElement.EnumerateObject().Single();
        return reason.Name switch
        {
            "version" => new Stale.Version(reason.Value.GetProperty("chosen").GetString()!),
            "settled" => new Stale.Settled(
                reason.Value.GetProperty("chosen").GetInt32(), reason.Value.GetProperty("now").GetInt32()),
            _ => throw new JsonException($"a stale reason is version, schema or settled, not {reason.Name}"),
        };
    }

    public override void Write(Utf8JsonWriter writer, Stale value, JsonSerializerOptions options)
    {
        switch (value)
        {
            case Stale.Schema:
                writer.WriteStringValue("schema");
                break;
            case Stale.Version version:
                writer.WriteStartObject();
                writer.WriteStartObject("version");
                writer.WriteString("chosen", version.Chosen);
                writer.WriteEndObject();
                writer.WriteEndObject();
                break;
            case Stale.Settled settled:
                writer.WriteStartObject();
                writer.WriteStartObject("settled");
                writer.WriteNumber("chosen", settled.Chosen);
                writer.WriteNumber("now", settled.Now);
                writer.WriteEndObject();
                writer.WriteEndObject();
                break;
        }
    }
}
