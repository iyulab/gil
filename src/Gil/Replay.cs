using System.Buffers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;

namespace Gil;

/// <summary>Choosing an attribute's thresholds by replaying settled records in settlement order, in the native core —
/// the records cross once, as their handle.</summary>
public static class Replay
{
    /// <summary>How many of the latest records replayed <see cref="ThresholdChoice.Promise"/> is taken over — every
    /// record in settlement order counts, whether or not it was asked about. The core's <c>RECENT_RECORDS</c>.</summary>
    public const int RecentRecords = 5_000;

    /// <summary>How many standard errors below the measured precision <see cref="ThresholdRule.Pooled"/> holds the target
    /// at. The core's <c>POOLED_STANDARD_ERRORS</c>.</summary>
    public const double PooledStandardErrors = 1.645;

    /// <summary>How many of the attribute's latest lookups <see cref="ThresholdRule.Pooled"/> chooses on, at the least.
    /// The core's <c>POOLED_RECENT_LOOKUPS</c>.</summary>
    public const int PooledRecentLookups = 5_000;

    /// <summary>How many answers, under the attribute's thresholds as they stand, <see cref="ThresholdRule.Pooled"/>
    /// reaches back for when its latest lookups hold fewer. The core's <c>POOLED_RECENT_ANSWERS</c>.</summary>
    public const int PooledRecentAnswers = 1_000;

    /// <summary>The key and similar record thresholds for <paramref name="attribute"/>, with the replay's decision
    /// records when <see cref="SelectionOptions.Decisions"/> asks for them.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute is not decided.</exception>
    public static LayerSelection SelectLayers(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Read(SelectLayersJson(schema, attribute, records, options), CoreJson.Default.LayerSelection);

    /// <summary>The thresholds of <paramref name="attribute"/>'s coarse level, as <see cref="SelectLayers"/> gives
    /// them.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute has no coarse level.</exception>
    public static LayerSelection SelectCoarse(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Read(SelectCoarseJson(schema, attribute, records, options), CoreJson.Default.LayerSelection);

    /// <summary>The observed attributes the key layer of <paramref name="attribute"/> should lean on, and the trials
    /// that chose them.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute is not decided.</exception>
    public static KeyFieldsChoice SelectKeyFields(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Read(SelectKeyFieldsJson(schema, attribute, records, options), CoreJson.Default.KeyFieldsChoice);

    /// <summary>How concentrated <paramref name="attribute"/>'s values are under each observed attribute's keys — what
    /// <see cref="Attribute.KeyConcentrations"/> takes.</summary>
    /// <exception cref="ArgumentException">The attribute is not decided.</exception>
    public static IReadOnlyDictionary<string, double> SelectKeyConcentrations(
        Schema schema, string attribute, SettledRecords records) =>
        Read(SelectKeyConcentrationsJson(schema, attribute, records), CoreJson.Default.IReadOnlyDictionaryStringDouble);

    private static T Read<T>(string json, JsonTypeInfo<T> type) =>
        JsonSerializer.Deserialize(json, type)!;

    /// <summary>The key and similar record thresholds for <paramref name="attribute"/>: <c>{"thresholds": …}</c>, with
    /// <c>"decisions"</c> when asked for.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute is not decided.</exception>
    internal static unsafe string SelectLayersJson(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Select(&Native.SelectLayers, schema, attribute, records, options);

    /// <summary>The thresholds of <paramref name="attribute"/>'s coarse level, as <see cref="SelectLayersJson"/> gives
    /// them.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute has no coarse level.</exception>
    internal static unsafe string SelectCoarseJson(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Select(&Native.SelectCoarse, schema, attribute, records, options);

    /// <summary>The observed attributes the key layer of <paramref name="attribute"/> should lean on, and the trials
    /// that chose them.</summary>
    /// <exception cref="ArgumentException">The options are out of range, or the attribute is not decided.</exception>
    internal static unsafe string SelectKeyFieldsJson(
        Schema schema, string attribute, SettledRecords records, SelectionOptions options) =>
        Select(&Native.SelectKeyFields, schema, attribute, records, options);

    /// <summary>How concentrated <paramref name="attribute"/>'s values are under each observed attribute's keys.</summary>
    /// <exception cref="ArgumentException">The attribute is not decided.</exception>
    internal static unsafe string SelectKeyConcentrationsJson(Schema schema, string attribute, SettledRecords records)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(records);
        var name = new Utf8(attribute);
        nuint length;
        fixed (byte* a = &name.Pin())
        {
            return Native.Result(Native.SelectKeyConcentrations(schema.Handle, name.At(a), records.Handle, &length));
        }
    }

    private static unsafe string Select(
        delegate*<SchemaHandle, Native.Str, RecordsHandle, Native.Str, nuint*, int> select, Schema schema, string attribute, SettledRecords records, SelectionOptions options)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(records);
        ArgumentNullException.ThrowIfNull(options);
        var name = new Utf8(attribute);
        var json = new Utf8(Options(options));
        nuint length;
        fixed (byte* a = &name.Pin())
        fixed (byte* o = &json.Pin())
        {
            return Native.Result(select(schema.Handle, name.At(a), records.Handle, json.At(o), &length));
        }
    }

    /// <summary>The options as the C ABI reads them.</summary>
    internal static string Options(SelectionOptions options)
    {
        var buffer = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            writer.WriteNumber("target_precision", options.TargetPrecision);
            writer.WriteNumber("minimum_answered", options.MinimumAnswered);
            if (options.Counting is { } counting)
            {
                writer.WriteStartArray("counting");
                foreach (var kind in counting)
                {
                    writer.WriteStringValue(kind.ToString());
                }

                writer.WriteEndArray();
            }

            writer.WriteString("rule", options.Rule == ThresholdRule.Pooled ? "pooled" : "every_band");
            writer.WriteBoolean("decisions", options.Decisions);
            writer.WriteNumber("typed_longest", options.TypedLongest);
            writer.WriteEndObject();
        }

        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }
}

/// <summary>A tuning through the C ABI, as JSON in the core's form.</summary>
internal static class TuningJson
{
    /// <summary>The chosen settings of the schema's decided attributes, chosen on <paramref name="settled"/> records for
    /// <paramref name="targetPrecision"/>.</summary>
    internal static unsafe string CaptureJson(Schema schema, int settled, double targetPrecision)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentOutOfRangeException.ThrowIfNegative(settled);
        nuint length;
        return Native.Result(Native.TuningCapture(schema.Handle, (nuint)settled, targetPrecision, &length));
    }

    /// <summary>The tuning with what <paramref name="attribute"/>'s layers selection promised — its
    /// <c>thresholds</c>, JSON as <see cref="Replay.SelectLayersJson"/> gives them.</summary>
    /// <exception cref="ArgumentException">The tuning or the thresholds do not read.</exception>
    internal static unsafe string PromiseJson(string tuningJson, string attribute, string thresholdsJson)
    {
        ArgumentNullException.ThrowIfNull(tuningJson);
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(thresholdsJson);
        var tuning = new Utf8(tuningJson);
        var name = new Utf8(attribute);
        var thresholds = new Utf8(thresholdsJson);
        nuint length;
        fixed (byte* t = &tuning.Pin())
        fixed (byte* a = &name.Pin())
        fixed (byte* h = &thresholds.Pin())
        {
            return Native.Result(Native.TuningPromise(tuning.At(t), name.At(a), thresholds.At(h), &length));
        }
    }

    /// <summary>A new schema: <paramref name="schema"/> with the tuning's settings put back on its attributes.</summary>
    /// <exception cref="ArgumentException">The tuning does not read, or the schema it makes breaks a schema rule.</exception>
    internal static unsafe Schema Apply(string tuningJson, Schema schema)
    {
        ArgumentNullException.ThrowIfNull(tuningJson);
        ArgumentNullException.ThrowIfNull(schema);
        var tuning = new Utf8(tuningJson);
        nint output;
        fixed (byte* t = &tuning.Pin())
        {
            Native.Check(Native.TuningApply(tuning.At(t), schema.Handle, &output));
        }

        return Schema.Own(output);
    }

    /// <summary>Why the tuning no longer stands for <paramref name="schema"/> with <paramref name="settled"/> records, as
    /// JSON — <c>null</c> while it does.</summary>
    /// <exception cref="ArgumentException">The tuning does not read.</exception>
    internal static unsafe string? StaleJson(string tuningJson, Schema schema, int settled)
    {
        ArgumentNullException.ThrowIfNull(tuningJson);
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentOutOfRangeException.ThrowIfNegative(settled);
        var tuning = new Utf8(tuningJson);
        nuint length;
        string found;
        fixed (byte* t = &tuning.Pin())
        {
            found = Native.Result(Native.TuningStale(tuning.At(t), schema.Handle, (nuint)settled, &length));
        }

        return found == "null" ? null : found;
    }
}

/// <summary>Counts of how a decided attribute's answers fared — content-free, for a host to report.</summary>
public static class Usage
{
    /// <summary>The counts over <paramref name="decisions"/>, per attribute.</summary>
    public static IReadOnlyDictionary<string, AttributeUsage> Count(IReadOnlyList<UsageDecision> decisions) =>
        JsonSerializer.Deserialize(CountJson(decisions), CoreJson.Default.IReadOnlyDictionaryStringAttributeUsage)!;

    /// <summary><see cref="Count"/> as JSON in the core's form.</summary>
    internal static unsafe string CountJson(IReadOnlyList<UsageDecision> decisions)
    {
        ArgumentNullException.ThrowIfNull(decisions);
        Native.EnsureCompatible();
        var texts = new List<string?>(decisions.Count * 4);
        foreach (var d in decisions)
        {
            ArgumentNullException.ThrowIfNull(d);
            texts.Add(d.Attribute);
            texts.Add(d.Answer);
            texts.Add(d.Settled);
            texts.Add(d.Settlement?.ToString());
        }

        var block = Encoding.UTF8.GetBytes(string.Concat(texts));
        var flat = new Native.Decision[Math.Max(decisions.Count, 1)];
        nuint length;
        fixed (byte* b = block)
        fixed (Native.Decision* f = flat)
        {
            var at = b;
            for (var i = 0; i < decisions.Count; i++)
            {
                var d = decisions[i];
                flat[i] = new()
                {
                    Attribute = Next(texts[i * 4], ref at),
                    Answer = Next(texts[(i * 4) + 1], ref at),
                    Settled = Next(texts[(i * 4) + 2], ref at),
                    Settlement = Next(texts[(i * 4) + 3], ref at),
                    Probability = d.Probability,
                    Promise = d.Promise ?? double.NaN,
                };
            }

            return Native.Result(Native.Usage(f, (nuint)decisions.Count, &length));
        }
    }

    /// <summary>The next text of the block <paramref name="at"/> points into, moving past it; null is null text.</summary>
    private static unsafe Native.Str Next(string? text, ref byte* at)
    {
        if (text is null)
        {
            return default;
        }

        var count = Encoding.UTF8.GetByteCount(text);
        var str = new Native.Str { Ptr = at, Len = (nuint)count };
        at += count;
        return str;
    }
}
