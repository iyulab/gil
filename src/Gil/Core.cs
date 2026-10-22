using System.Buffers;
using System.Text;
using System.Text.Json;

namespace Gil;

/// <summary>A schema checked once: its name and attributes, as the core keeps them — the settings an attribute leaves out
/// filled with their defaults. Schemas, records and batch results also cross as JSON in the core's form.</summary>
public sealed class Schema : IDisposable
{
    private Schema(SchemaHandle handle)
    {
        Handle = handle;
        var form = JsonSerializer.Deserialize(ToJson(), CoreJson.Default.SchemaForm)!;
        Name = form.Name;
        Attributes = form.Attributes;
    }

    /// <summary>A schema named <paramref name="name"/> with <paramref name="attributes"/>, in order.</summary>
    /// <exception cref="ArgumentException">The schema does not check; the message says why.</exception>
    public Schema(string name, IEnumerable<AttributeDefinition> attributes)
        : this(Check(name, attributes))
    {
    }

    internal SchemaHandle Handle { get; }

    /// <summary>The schema's name.</summary>
    public string Name { get; }

    /// <summary>The schema's attributes, in order, as the core keeps them.</summary>
    public IReadOnlyList<AttributeDefinition> Attributes { get; }

    /// <summary>The attribute named <paramref name="name"/>, or null when the schema has none.</summary>
    public AttributeDefinition? Find(string name) => Attributes.FirstOrDefault(a => a.Name == name);

    /// <summary>A schema from JSON in the core's form; the settings an attribute leaves out take their defaults.</summary>
    /// <exception cref="ArgumentException">The JSON does not read as a schema, or the schema does not check.</exception>
    public static Schema FromJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        return new Schema(New(json));
    }

    internal static Schema Own(nint handle) => new(SchemaHandle.Of(handle));

    /// <summary>A new schema: this one with what a layers selection chose (<see cref="Replay.SelectLayers"/>) put on the
    /// decided attribute <paramref name="attribute"/> — the key threshold, the memory threshold and its similarity floor;
    /// a layer with no chosen threshold is left unset, so it does not answer the attribute.</summary>
    /// <exception cref="ArgumentException">The schema has no such decided attribute, or the schema it makes breaks a
    /// schema rule.</exception>
    public unsafe Schema WithLayers(string attribute, LayerThresholds layers) =>
        With(&Native.SchemaWithLayers, attribute, layers);

    /// <summary>As <see cref="WithLayers"/>, for what a coarse selection (<see cref="Replay.SelectCoarse"/>) chose: the
    /// thresholds go on the attribute's coarse level.</summary>
    /// <exception cref="ArgumentException">As <see cref="WithLayers"/>, or the attribute has no coarse level.</exception>
    public unsafe Schema WithCoarse(string attribute, LayerThresholds layers) =>
        With(&Native.SchemaWithCoarse, attribute, layers);

    /// <summary>The schema as JSON in the core's form.</summary>
    public unsafe string ToJson()
    {
        nuint length;
        return Native.Result(Native.SchemaJson(Handle, &length));
    }

    /// <inheritdoc/>
    public void Dispose() => Handle.Dispose();

    private unsafe Schema With(
        delegate*<SchemaHandle, Native.Str, Native.Str, nint*, int> put, string attribute, LayerThresholds layers)
    {
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(layers);
        var name = new Utf8(attribute);
        var thresholds = new Utf8(JsonSerializer.Serialize(layers, CoreJson.Default.LayerThresholds));
        nint output;
        fixed (byte* a = &name.Pin())
        fixed (byte* t = &thresholds.Pin())
        {
            Native.Check(put(Handle, name.At(a), thresholds.At(t), &output));
        }

        return Own(output);
    }

    private static SchemaHandle Check(string name, IEnumerable<AttributeDefinition> attributes)
    {
        ArgumentNullException.ThrowIfNull(name);
        ArgumentNullException.ThrowIfNull(attributes);
        var form = new SchemaForm(name, [.. attributes]);
        foreach (var attribute in form.Attributes)
        {
            ArgumentNullException.ThrowIfNull(attribute, nameof(attributes));
        }

        return New(JsonSerializer.Serialize(form, CoreJson.Default.SchemaForm));
    }

    private static unsafe SchemaHandle New(string json)
    {
        Native.EnsureCompatible();
        var text = new Utf8(json);
        nint output;
        fixed (byte* p = &text.Pin())
        {
            Native.Check(Native.SchemaNew(text.At(p), &output));
        }

        return SchemaHandle.Of(output);
    }
}

/// <summary>Settled records of a schema, in settlement order, read once — what fills a decider and what choosing
/// thresholds replays.</summary>
public sealed class SettledRecords : IDisposable
{
    private SettledRecords(RecordsHandle handle) => Handle = handle;

    internal RecordsHandle Handle { get; }

    /// <summary>How many records it holds.</summary>
    public int Count => (int)Native.RecordsLen(Handle);

    /// <summary>Records from JSON lines, one record per line in the core's form (<c>settled_at</c> in 100-nanosecond
    /// ticks since the Unix epoch, UTC, or as a time in the round-trip format <c>"O"</c>; an entry given as null is left
    /// out); blank lines are skipped.</summary>
    /// <exception cref="ArgumentException">A line does not read as a record.</exception>
    public static unsafe SettledRecords FromJsonLines(string lines)
    {
        ArgumentNullException.ThrowIfNull(lines);
        Native.EnsureCompatible();
        var text = new Utf8(lines);
        nint output;
        fixed (byte* p = &text.Pin())
        {
            Native.Check(Native.RecordsNew(text.At(p), &output));
        }

        return new SettledRecords(RecordsHandle.Of(output));
    }

    /// <summary><paramref name="records"/>, in settlement order.</summary>
    /// <exception cref="ArgumentException">A record does not read; the message says why.</exception>
    public static SettledRecords From(IEnumerable<Record> records)
    {
        ArgumentNullException.ThrowIfNull(records);
        var lines = new StringBuilder();
        foreach (var record in records)
        {
            ArgumentNullException.ThrowIfNull(record, nameof(records));
            lines.Append(JsonSerializer.Serialize(record, CoreJson.Default.Record)).Append('\n');
        }

        return FromJsonLines(lines.ToString());
    }

    /// <inheritdoc/>
    public void Dispose() => Handle.Dispose();
}

/// <summary>The memories that answer: the settled key memory and, unless left out, the similar record memory. Answers
/// from one decider run in parallel; a call that changes it — putting records, observing or settling a record — waits
/// for them and runs alone.</summary>
public sealed class Decider : IDisposable
{
    internal readonly ReaderWriterLockSlim Gate = new();

    /// <summary>A decider with empty memories.</summary>
    /// <param name="similarRecords">Whether to keep the similar record memory.</param>
    public unsafe Decider(bool similarRecords = true)
    {
        Native.EnsureCompatible();
        nint output;
        Native.Check(Native.DeciderNew(similarRecords ? (byte)1 : (byte)0, &output));
        Handle = DeciderHandle.Of(output);
    }

    internal DeciderHandle Handle { get; }

    /// <summary>Puts settled records of <paramref name="schema"/> into the memories — records settled later than
    /// those they hold.</summary>
    public void Put(Schema schema, SettledRecords records)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(records);
        using (Gate.Writing())
        {
            Native.Check(Native.DeciderPut(Handle, schema.Handle, records.Handle), changing: true);
        }
    }

    /// <summary>Takes record <paramref name="recordId"/> out of the memories — a record the host deleted; nothing
    /// happens when they do not hold it. The key memory is then as if the record had never been put, and the similar
    /// record memory never offers it again.</summary>
    public unsafe void Remove(Schema schema, string recordId)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(recordId);
        var id = new Utf8(recordId);
        using (Gate.Writing())
        {
            fixed (byte* i = &id.Pin())
            {
                Native.Check(Native.DeciderRemove(Handle, schema.Handle, id.At(i)), changing: true);
            }
        }
    }

    /// <summary>The answers for every open decided attribute of record <paramref name="recordId"/> from the values
    /// given, writing nothing to memory, in the schema's order — each with its lookup's decision record when
    /// <paramref name="decisions"/> is true.</summary>
    public IReadOnlyList<Answer> Answer(
        Schema schema,
        string recordId,
        IReadOnlyDictionary<string, string> values,
        IReadOnlyDictionary<string, IReadOnlyList<string>>? sets = null,
        bool decisions = false) =>
        Answers.Read(AnswerJson(schema, recordId, values, sets, decisions));

    /// <summary><see cref="Answer"/> as a JSON array in the core's form (each answer with <c>answered</c>, and with
    /// <c>decision</c> — the lookup's decision record — when <paramref name="decisions"/> is true).</summary>
    public unsafe string AnswerJson(
        Schema schema,
        string recordId,
        IReadOnlyDictionary<string, string> values,
        IReadOnlyDictionary<string, IReadOnlyList<string>>? sets = null,
        bool decisions = false)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(recordId);
        ArgumentNullException.ThrowIfNull(values);
        var id = new Utf8(recordId);
        var valuesJson = new Utf8(Json.Object(values));
        var setsJson = new Utf8(sets is null ? string.Empty : Json.Sets(sets));
        nuint length;
        using (Gate.Reading())
        {
            fixed (byte* i = &id.Pin())
            fixed (byte* v = &valuesJson.Pin())
            fixed (byte* s = &setsJson.Pin())
            {
                return Native.Result(Native.DeciderAnswer(
                    Handle, schema.Handle, id.At(i), valuesJson.At(v), setsJson.At(s), decisions ? (byte)1 : (byte)0,
                    &length));
            }
        }
    }

    /// <inheritdoc/>
    public void Dispose()
    {
        Handle.Dispose();
        Gate.Dispose();
    }
}

/// <summary>One record of a schema while its values arrive. Each change puts it into the decider's memory again.
/// Calls on one record are serialized; they hold the decider's lock while they use it — shared to answer, alone to
/// change it.</summary>
public sealed class OpenRecord : IDisposable
{
    private readonly Lock _gate = new();
    private Native.Candidate[] _slots = new Native.Candidate[8];
    private byte[] _bytes = new byte[256];

    private OpenRecord(OpenRecordHandle handle, Schema schema)
    {
        Handle = handle;
        Schema = schema;
    }

    internal OpenRecordHandle Handle { get; }

    /// <summary>The schema the record is of.</summary>
    public Schema Schema { get; }

    /// <summary>A new record <paramref name="id"/> of <paramref name="schema"/>, starting at
    /// <paramref name="settledAt"/> — the present for a new record.</summary>
    /// <exception cref="ArgumentException">The id is blank.</exception>
    public static unsafe OpenRecord Open(Schema schema, string id, DateTimeOffset settledAt)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(id);
        var text = new Utf8(id);
        nint output;
        fixed (byte* p = &text.Pin())
        {
            Native.Check(Native.OpenRecordNew(schema.Handle, text.At(p), Time.Ticks(settledAt), &output));
        }

        return new OpenRecord(OpenRecordHandle.Of(output), schema);
    }

    /// <summary>A saved record — as <see cref="Snapshot"/> gives it — reopened as it was left.</summary>
    public static OpenRecord Reopen(Schema schema, Record saved)
    {
        ArgumentNullException.ThrowIfNull(saved);
        return Reopen(schema, JsonSerializer.Serialize(saved, CoreJson.Default.Record));
    }

    /// <summary>A saved record — JSON as <see cref="SnapshotJson"/> gives it — reopened as it was left.</summary>
    public static unsafe OpenRecord Reopen(Schema schema, string snapshotJson)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(snapshotJson);
        var text = new Utf8(snapshotJson);
        nint output;
        fixed (byte* p = &text.Pin())
        {
            Native.Check(Native.OpenRecordReopen(schema.Handle, text.At(p), &output));
        }

        return new OpenRecord(OpenRecordHandle.Of(output), schema);
    }

    /// <summary>Sets an observed attribute's value — or clears it, given null — and returns the fresh answers it
    /// causes.</summary>
    /// <exception cref="ArgumentException">The attribute is not observed.</exception>
    public IReadOnlyList<Answer> Observe(Decider decider, string attribute, string? value) =>
        Answers.Read(ObserveJson(decider, attribute, value));

    private unsafe string ObserveJson(Decider decider, string attribute, string? value)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(attribute);
        var name = new Utf8(attribute);
        var text = new Utf8(value);
        nuint length;
        lock (_gate)
        using (decider.Gate.Writing())
        {
            fixed (byte* a = &name.Pin())
            fixed (byte* v = &text.Pin())
            {
                return Native.Result(
                    Native.OpenRecordObserve(Handle, decider.Handle, name.At(a), text.At(v), &length),
                    changing: true);
            }
        }
    }

    /// <summary>Settles a decided attribute at <paramref name="now"/> with the parts its kind needs — a value for the
    /// kinds that settle one, values for <see cref="SettlementKind.Set"/> (and for a restore of an attribute that takes
    /// several), the probability a check was drawn with — and returns the fresh answers it causes.</summary>
    /// <exception cref="ArgumentException">A part the kind needs is missing, or the attribute is not decided or takes
    /// the other kind of value.</exception>
    public IReadOnlyList<Answer> Settle(
        Decider decider,
        string attribute,
        SettlementKind kind,
        DateTimeOffset now,
        string? value = null,
        IReadOnlyList<string>? values = null,
        double probability = 1.0) =>
        Answers.Read(SettleJson(decider, attribute, kind, now, value, values, probability));

    private unsafe string SettleJson(
        Decider decider,
        string attribute,
        SettlementKind kind,
        DateTimeOffset now,
        string? value = null,
        IReadOnlyList<string>? values = null,
        double probability = 1.0)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(attribute);
        var name = new Utf8(attribute);
        var kindName = new Utf8(kind.ToString());
        var text = new Utf8(value);
        var block = values is null ? null : Encoding.UTF8.GetBytes(string.Concat(values));
        // Never empty when given, so that an empty list pins to a real address: null is no values, a pointer to none
        // is an empty set.
        var strs = values is null ? null : new Native.Str[Math.Max(values.Count, 1)];
        nuint length;
        lock (_gate)
        using (decider.Gate.Writing())
        {
            fixed (byte* a = &name.Pin())
            fixed (byte* k = &kindName.Pin())
            fixed (byte* v = &text.Pin())
            fixed (byte* b = block)
            fixed (Native.Str* s = strs)
            {
                if (values is not null)
                {
                    var at = 0;
                    for (var i = 0; i < values.Count; i++)
                    {
                        var count = Encoding.UTF8.GetByteCount(values[i]);
                        strs![i] = new() { Ptr = b + at, Len = (nuint)count };
                        at += count;
                    }
                }

                return Native.Result(
                    Native.OpenRecordSettle(
                        Handle, decider.Handle, name.At(a), kindName.At(k), Time.Ticks(now), text.At(v), s,
                        (nuint)(values?.Count ?? 0), probability, &length),
                    changing: true);
            }
        }
    }

    /// <summary>The answers for every open decided attribute — each with its lookup's decision record when
    /// <paramref name="decisions"/> is true.</summary>
    public IReadOnlyList<Answer> Answer(Decider decider, bool decisions = false) =>
        Answers.Read(AnswerJson(decider, decisions));

    /// <summary>The answers for every open decided attribute, with <paramref name="judge"/>'s judgement in those that
    /// ask for one (<see cref="Answer.NeedsExternal"/>): it is asked for each of them at once, and what it ranks is offered
    /// back (<see cref="OfferExternal"/>) — unless it abstains, or the record changed meanwhile so that the attribute no
    /// longer asks on the same values, when the answer is left as the record now gives it. The core's work is
    /// synchronous; only the judge is awaited.</summary>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> was canceled.</exception>
    public async Task<IReadOnlyList<Answer>> AnswerAsync(
        Decider decider, IExternalJudge judge, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(judge);
        var asking = Answer(decider).Where(a => a.NeedsExternal is not null).ToList();
        if (asking.Count == 0)
        {
            return Answer(decider);
        }

        var judged = await Task.WhenAll(asking.Select(async a =>
        {
            var attribute = Schema.Find(a.Attribute)!;
            var ranked = await judge.JudgeAsync(attribute, a.NeedsExternal!, cancellationToken).ConfigureAwait(false);
            return (a.Attribute, Evidence: a.NeedsExternal!, Ranked: ranked);
        })).ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();

        var byAttribute = judged.Where(j => j.Ranked.Count > 0).ToDictionary(j => j.Attribute);
        return [.. Answer(decider).Select(a =>
            byAttribute.TryGetValue(a.Attribute, out var j) && a.NeedsExternal is { } now && now.SequenceEqual(j.Evidence)
                ? OfferExternal(decider, a.Attribute, j.Ranked)
                : a)];
    }

    /// <summary><see cref="Answer(Decider, bool)"/> as a JSON array in the core's form.</summary>
    public unsafe string AnswerJson(Decider decider, bool decisions = false)
    {
        ArgumentNullException.ThrowIfNull(decider);
        nuint length;
        lock (_gate)
        using (decider.Gate.Reading())
        {
            return Native.Result(
                Native.OpenRecordAnswer(Handle, decider.Handle, decisions ? (byte)1 : (byte)0, &length));
        }
    }

    /// <summary>The answer for one open decided attribute alone, with <paramref name="partial"/> the text typed into it
    /// so far — read without JSON.</summary>
    /// <exception cref="ArgumentException">The attribute is not open: not decided, off, settled or rejected.</exception>
    public unsafe PartialAnswer AnswerPartial(Decider decider, string attribute, string? partial = null)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(attribute);
        var name = new Utf8(attribute);
        var typed = new Utf8(partial);
        lock (_gate)
        using (decider.Gate.Reading())
        {
            while (true)
            {
                Native.Answer head;
                int status;
                fixed (byte* a = &name.Pin())
                fixed (byte* t = &typed.Pin())
                fixed (Native.Candidate* c = _slots)
                fixed (byte* b = _bytes)
                {
                    status = Native.OpenRecordAnswerPartial(
                        Handle, decider.Handle, name.At(a), typed.At(t), &head, c, (nuint)_slots.Length, b,
                        (nuint)_bytes.Length);
                }

                if (status == Native.BufferTooSmall)
                {
                    _slots = new Native.Candidate[Math.Max((int)head.Candidates, _slots.Length)];
                    _bytes = new byte[Math.Max((int)head.ValueBytes, _bytes.Length)];
                    continue;
                }

                Native.Check(status);
                var candidates = new Candidate[head.Candidates];
                for (var i = 0; i < candidates.Length; i++)
                {
                    candidates[i] = Read(_slots[i]);
                }

                return new PartialAnswer(
                    (Source)head.Source,
                    (AttributePolicy)head.Policy,
                    head.Answered != 0,
                    head.NeedsExternal != 0,
                    candidates,
                    head.HasCoarse != 0 ? Read(head.Coarse) : null);
            }
        }
    }

    /// <summary>The answer for an open decided attribute with an external judgement's ranked values in it.</summary>
    public Answer OfferExternal(Decider decider, string attribute, IReadOnlyList<ExternalCandidate> ranked) =>
        JsonSerializer.Deserialize(OfferExternalJson(decider, attribute, ranked), CoreJson.Default.Answer)!;

    private unsafe string OfferExternalJson(Decider decider, string attribute, IReadOnlyList<ExternalCandidate> ranked)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(ranked);
        var name = new Utf8(attribute);
        var block = Encoding.UTF8.GetBytes(string.Concat(ranked.Select(r => r.Value)));
        var flat = new Native.ExternalCandidate[ranked.Count];
        nuint length;
        lock (_gate)
        using (decider.Gate.Reading())
        {
            fixed (byte* a = &name.Pin())
            fixed (byte* b = block)
            fixed (Native.ExternalCandidate* r = flat)
            {
                var at = 0;
                for (var i = 0; i < ranked.Count; i++)
                {
                    var count = Encoding.UTF8.GetByteCount(ranked[i].Value);
                    flat[i] = new()
                    {
                        Value = new() { Ptr = b + at, Len = (nuint)count },
                        Probability = ranked[i].Probability,
                        Trusted = ranked[i].Trusted ? (byte)1 : (byte)0,
                    };
                    at += count;
                }

                return Native.Result(Native.OpenRecordOfferExternal(
                    Handle, decider.Handle, name.At(a), r, (nuint)flat.Length, &length));
            }
        }
    }

    /// <summary>The record as it stands, to save and reopen, or to put as settled.</summary>
    public Record Snapshot() => JsonSerializer.Deserialize(SnapshotJson(), CoreJson.Default.Record)!;

    /// <summary>The record as it stands, to save — JSON in the core's form, as records are read and reopened.</summary>
    public unsafe string SnapshotJson()
    {
        nuint length;
        lock (_gate)
        {
            return Native.Result(Native.OpenRecordSnapshot(Handle, &length));
        }
    }

    /// <inheritdoc/>
    public void Dispose() => Handle.Dispose();

    private Candidate Read(Native.Candidate c) => new(
        Encoding.UTF8.GetString(_bytes, (int)c.ValueOffset, (int)c.ValueLen),
        c.Score,
        (Source)c.Source,
        c.Trusted != 0);
}

/// <summary>Answers read from the core's JSON.</summary>
internal static class Answers
{
    public static IReadOnlyList<Answer> Read(string json) => JsonSerializer.Deserialize(json, CoreJson.Default.IReadOnlyListAnswer)!;
}

/// <summary>Scopes for a decider's reader–writer lock.</summary>
internal static class Gates
{
    public static Scope Reading(this ReaderWriterLockSlim gate)
    {
        gate.EnterReadLock();
        return new Scope(gate, write: false);
    }

    public static Scope Writing(this ReaderWriterLockSlim gate)
    {
        gate.EnterWriteLock();
        return new Scope(gate, write: true);
    }

    public readonly struct Scope(ReaderWriterLockSlim gate, bool write) : IDisposable
    {
        public void Dispose()
        {
            if (write)
            {
                gate.ExitWriteLock();
            }
            else
            {
                gate.ExitReadLock();
            }
        }
    }
}

/// <summary>The JSON the C ABI reads, written without reflection.</summary>
internal static class Json
{
    public static string Object(IReadOnlyDictionary<string, string> values)
    {
        var buffer = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            foreach (var (name, value) in values)
            {
                writer.WriteString(name, value);
            }

            writer.WriteEndObject();
        }

        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }

    public static string Sets(IReadOnlyDictionary<string, IReadOnlyList<string>> sets)
    {
        var buffer = new ArrayBufferWriter<byte>();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            foreach (var (name, values) in sets)
            {
                writer.WriteStartArray(name);
                foreach (var value in values)
                {
                    writer.WriteStringValue(value);
                }

                writer.WriteEndArray();
            }

            writer.WriteEndObject();
        }

        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }
}
