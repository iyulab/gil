using System.Buffers;
using System.Text;
using System.Text.Json;

namespace Gil.Native;

/// <summary>A schema read and checked once. Schemas, records and batch results cross as JSON in the core's form.</summary>
public sealed class Schema : IDisposable
{
    private Schema(SchemaHandle handle) => Handle = handle;

    internal SchemaHandle Handle { get; }

    /// <summary>A schema from JSON in the core's form; the settings an attribute leaves out take their defaults.</summary>
    /// <exception cref="ArgumentException">The JSON does not read as a schema, or the schema does not check.</exception>
    public static unsafe Schema FromJson(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        Native.EnsureCompatible();
        var text = new Utf8(json);
        nint output;
        fixed (byte* p = &text.Pin())
        {
            Native.Check(Native.SchemaNew(text.At(p), &output));
        }

        return new Schema(SchemaHandle.Of(output));
    }

    internal static Schema Own(nint handle) => new(SchemaHandle.Of(handle));

    /// <summary>The schema as JSON in the core's form.</summary>
    public unsafe string ToJson()
    {
        nuint length;
        return Native.Result(Native.SchemaJson(Handle, &length));
    }

    /// <inheritdoc/>
    public void Dispose() => Handle.Dispose();
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
    /// ticks since the Unix epoch, UTC); blank lines are skipped.</summary>
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

    /// <summary>The answers for every open decided attribute of record <paramref name="recordId"/> from the values
    /// given, writing nothing to memory, as a JSON array in the core's form (each answer with <c>answered</c>).</summary>
    public unsafe string AnswerJson(
        Schema schema,
        string recordId,
        IReadOnlyDictionary<string, string> values,
        IReadOnlyDictionary<string, IReadOnlyList<string>>? sets = null)
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
                    Handle, schema.Handle, id.At(i), valuesJson.At(v), setsJson.At(s), &length));
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

    private OpenRecord(OpenRecordHandle handle) => Handle = handle;

    internal OpenRecordHandle Handle { get; }

    /// <summary>100-nanosecond ticks since the Unix epoch, UTC — the core's time.</summary>
    public static long Ticks(DateTimeOffset time) => time.UtcTicks - DateTimeOffset.UnixEpoch.UtcTicks;

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
            Native.Check(Native.OpenRecordNew(schema.Handle, text.At(p), Ticks(settledAt), &output));
        }

        return new OpenRecord(OpenRecordHandle.Of(output));
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

        return new OpenRecord(OpenRecordHandle.Of(output));
    }

    /// <summary>Sets an observed attribute's value — or clears it, given null — and returns the fresh answers it
    /// causes, as a JSON array.</summary>
    /// <exception cref="ArgumentException">The attribute is not observed.</exception>
    public unsafe string Observe(Decider decider, string attribute, string? value)
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
    /// several), the probability a check was drawn with — and returns the fresh answers it causes, as a JSON
    /// array.</summary>
    /// <exception cref="ArgumentException">A part the kind needs is missing, or the attribute is not decided or takes
    /// the other kind of value.</exception>
    public unsafe string Settle(
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
                        Handle, decider.Handle, name.At(a), kindName.At(k), Ticks(now), text.At(v), s,
                        (nuint)(values?.Count ?? 0), probability, &length),
                    changing: true);
            }
        }
    }

    /// <summary>The answers for every open decided attribute, as a JSON array.</summary>
    public unsafe string AnswerJson(Decider decider)
    {
        ArgumentNullException.ThrowIfNull(decider);
        nuint length;
        lock (_gate)
        using (decider.Gate.Reading())
        {
            return Native.Result(Native.OpenRecordAnswer(Handle, decider.Handle, &length));
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
                    (Layer)head.Source,
                    (AnswerPolicy)head.Policy,
                    head.Answered != 0,
                    head.NeedsExternal != 0,
                    candidates,
                    head.HasCoarse != 0 ? Read(head.Coarse) : null);
            }
        }
    }

    /// <summary>The answer for an open decided attribute with an external judgement's ranked values in it, as JSON.
    /// </summary>
    public unsafe string OfferExternal(Decider decider, string attribute, IReadOnlyList<ExternalCandidate> ranked)
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
        (Layer)c.Source,
        c.Trusted != 0);
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
