using System.Text.Json;

namespace Gil;

/// <summary>The promise-keeping loop as one object: settled records go in once, the decider learns them, and the
/// thresholds are chosen again when they are stale — on a replay carried on from where the last choice left it, not from
/// the start. It holds the records and, for each decided attribute that takes one value, the replay as it stands.
/// <see cref="Settle"/> puts records into the decider and into the next choice, <see cref="Stale"/> says when the choice
/// no longer stands, and <see cref="Rechoose"/> replays only the records settled since and chooses again — the same
/// thresholds, to the bit, as <see cref="Replay.SelectLayers"/> on all the records. Answer with the decider and
/// <see cref="Schema"/>; an answer's promise is <see cref="Tuning"/>'s <see cref="Gil.Tuning.PromiseOf"/>. Give it the
/// same decider every time. Calls on one keeper run one at a time. Nothing is stored outside the process; the coarse
/// level is not chosen by it.</summary>
public sealed class Keeper : IDisposable
{
    private readonly Lock sync = new();

    /// <summary>A keeper of <paramref name="schema"/> with nothing settled, choosing as <paramref name="options"/>
    /// says (its target, fewest answers, counted kinds, rule and threads; <see cref="SelectionOptions.Decisions"/> and
    /// <see cref="SelectionOptions.TypedLongest"/> are not used). The thresholds <paramref name="schema"/> holds stand
    /// until the first <see cref="Rechoose"/>.</summary>
    /// <exception cref="ArgumentException">The options are out of range.</exception>
    public unsafe Keeper(Schema schema, SelectionOptions options)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(options);
        Native.EnsureCompatible();
        var json = new Utf8(Replay.Options(options));
        nint output;
        fixed (byte* o = &json.Pin())
        {
            Native.Check(Native.KeeperNew(schema.Handle, json.At(o), &output));
        }

        Handle = KeeperHandle.Of(output);
    }

    internal KeeperHandle Handle { get; }

    /// <summary>How many records were settled, a record settled again counting again — what staleness is counted
    /// on.</summary>
    public int Settled
    {
        get
        {
            lock (sync)
            {
                return (int)Native.KeeperSettled(Handle);
            }
        }
    }

    /// <summary>The schema as chosen: the one given, with the thresholds the last <see cref="Rechoose"/> put on its
    /// attributes — a new schema each time, for the caller to dispose.</summary>
    public unsafe Schema Schema
    {
        get
        {
            nint output;
            lock (sync)
            {
                Native.Check(Native.KeeperSchema(Handle, &output));
            }

            return Schema.Own(output);
        }
    }

    /// <summary>The last choice as a tuning, with each attribute's promises.</summary>
    public Tuning Tuning => State().Tuning;

    /// <summary>Puts settled records into <paramref name="decider"/> and into the next choice. Records may come in any
    /// order; one settled before the last a replay has gone through has the next choice replay from the start.</summary>
    /// <exception cref="ArgumentException">A record keeps an attribute where its kind does not belong.</exception>
    public void Settle(Decider decider, SettledRecords records)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(records);
        lock (sync)
        using (decider.Gate.Writing())
        {
            Native.Check(Native.KeeperSettle(Handle, decider.Handle, records.Handle), changing: true);
        }
    }

    /// <summary>Takes record <paramref name="recordId"/>, which the host deleted, out of <paramref name="decider"/> and
    /// out of the records; the next choice replays from the start without it.</summary>
    public unsafe void Remove(Decider decider, string recordId)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(recordId);
        var id = new Utf8(recordId);
        lock (sync)
        using (decider.Gate.Writing())
        {
            fixed (byte* i = &id.Pin())
            {
                Native.Check(Native.KeeperRemove(Handle, decider.Handle, id.At(i)), changing: true);
            }
        }
    }

    /// <summary>Why the last choice no longer stands — a tenth more or fewer records than it was chosen on — or null
    /// while it does.</summary>
    public unsafe Stale? Stale()
    {
        string found;
        nuint length;
        lock (sync)
        {
            found = Native.Result(Native.KeeperStale(Handle, &length));
        }

        return found == "null" ? null : JsonSerializer.Deserialize(found, CoreJson.Default.Stale);
    }

    /// <summary>Chooses every attribute's thresholds again on all the records settled — replaying only those settled
    /// since the last choice where it can — puts them on <see cref="Schema"/> and captures <see cref="Tuning"/> with
    /// their promises.</summary>
    /// <param name="decisions">Whether to keep the decision records of the records replayed this time.</param>
    public unsafe KeeperChoice Rechoose(bool decisions = false)
    {
        string json;
        nuint length;
        lock (sync)
        {
            json = Native.Result(Native.KeeperRechoose(Handle, decisions ? (byte)1 : (byte)0, &length), changing: true);
        }

        return JsonSerializer.Deserialize(json, CoreJson.Default.KeeperChoice)!;
    }

    /// <summary>The choice as it stands: the tuning and each attribute's thresholds, empty before the first
    /// <see cref="Rechoose"/>.</summary>
    public unsafe KeeperChoice State()
    {
        string json;
        nuint length;
        lock (sync)
        {
            json = Native.Result(Native.KeeperState(Handle, &length));
        }

        return JsonSerializer.Deserialize(json, CoreJson.Default.KeeperChoice)!;
    }

    /// <inheritdoc/>
    public void Dispose() => Handle.Dispose();
}

/// <summary>A keeper's choice: the tuning with its promises, each attribute's thresholds with how they did on the
/// replay, and — when asked for — the decision records of the records replayed this time.</summary>
public sealed record KeeperChoice(
    Tuning Tuning,
    IReadOnlyDictionary<string, LayerThresholds> Chosen,
    IReadOnlyList<DecisionRecord>? Decisions = null);
