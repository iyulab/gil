using System.Text;
using System.Text.Json;

namespace Gil;

/// <summary>Whether to ask a person for one observed attribute of an open record.</summary>
/// <param name="Attribute">The observed attribute.</param>
/// <param name="Gain">What asking it has gained per character typed, in records whose answer showed the same
/// class.</param>
/// <param name="Ask">Whether it is worth asking within the budget.</param>
/// <param name="Threshold">The gain per character asking must reach now; null before anything is learned.</param>
[Experimental(Stability.Asking, UrlFormat = Stability.Url)]
public sealed record AskAdvice(string Attribute, double Gain, bool Ask, double? Threshold);

/// <summary>Which observed attribute to ask a person for first, so that a decided attribute gets answered: for each
/// attribute that could be asked, how much asking it has gained per character typed in records like this one, and
/// whether it is worth asking within a budget — a share of the typing asking every time would take. It learns from
/// settled records in settlement order; each attribute's gain is learned with only that attribute missing, so advice for
/// more than one open at once is an approximation. Advice may be asked from several threads at once; learning waits for
/// it and runs alone.</summary>
[Experimental(Stability.Asking, UrlFormat = Stability.Url)]
public sealed class AskPlanner : IDisposable
{
    private readonly ReaderWriterLockSlim _gate = new();
    private readonly AskPlannerHandle _handle;

    /// <summary>A planner for the decided single-valued attribute <paramref name="attribute"/> of
    /// <paramref name="schema"/> (not off), the observed attributes <paramref name="askable"/> a person could be asked
    /// for (at least one, none twice), and the share <paramref name="budget"/> (above 0, at most 1) of the typing asking
    /// every attribute every time would take that asking may spend.</summary>
    /// <exception cref="ArgumentException">A rule is broken; the message names the first.</exception>
    public unsafe AskPlanner(Schema schema, string attribute, IReadOnlyList<string> askable, double budget)
    {
        ArgumentNullException.ThrowIfNull(schema);
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(askable);
        var name = new Utf8(attribute);
        var block = Encoding.UTF8.GetBytes(string.Concat(askable));
        var strs = new Native.Str[Math.Max(askable.Count, 1)];
        nint output;
        fixed (byte* a = &name.Pin())
        fixed (byte* b = block)
        fixed (Native.Str* s = strs)
        {
            var at = 0;
            for (var i = 0; i < askable.Count; i++)
            {
                ArgumentNullException.ThrowIfNull(askable[i], nameof(askable));
                var count = Encoding.UTF8.GetByteCount(askable[i]);
                strs[i] = new() { Ptr = b + at, Len = (nuint)count };
                at += count;
            }

            Native.Check(Native.AskPlannerNew(schema.Handle, name.At(a), s, (nuint)askable.Count, budget, &output));
        }

        _handle = AskPlannerHandle.Of(output);
        Attribute = attribute;
        Askable = [.. askable];
        Budget = budget;
    }

    /// <summary>The decided attribute it plans for.</summary>
    public string Attribute { get; }

    /// <summary>The attributes a person could be asked for, as given.</summary>
    public IReadOnlyList<string> Askable { get; }

    /// <summary>The share of the typing asking every time would take that asking may spend.</summary>
    public double Budget { get; }

    /// <summary>Learns from <paramref name="records"/>, in order — settled records of the planner's schema, in
    /// settlement order — asking <paramref name="decider"/> for the decided attribute's answer with and without each
    /// askable attribute a record filled. The decider's memories may hold a record being learned, but none settled
    /// after it.</summary>
    public void Learn(Decider decider, SettledRecords records)
    {
        ArgumentNullException.ThrowIfNull(decider);
        ArgumentNullException.ThrowIfNull(records);
        using (_gate.Writing())
        using (decider.Gate.Reading())
        {
            Native.Check(Native.AskPlannerLearn(_handle, decider.Handle, records.Handle), changing: true);
        }
    }

    /// <summary>Advice for an open record: every askable attribute it has no value for, by gain per character, highest
    /// first, each marked whether to ask. <paramref name="answer"/> is the decided attribute's answer for the record as
    /// it stands, <paramref name="values"/> its values as they stand.</summary>
    /// <exception cref="ArgumentException">The answer is for another attribute.</exception>
    public unsafe IReadOnlyList<AskAdvice> Advise(Answer answer, IReadOnlyDictionary<string, string> values)
    {
        ArgumentNullException.ThrowIfNull(answer);
        ArgumentNullException.ThrowIfNull(values);
        var answerJson = new Utf8(JsonSerializer.Serialize(answer, CoreJson.Default.Answer));
        var valuesJson = new Utf8(Json.Object(values));
        nuint length;
        string found;
        using (_gate.Reading())
        {
            fixed (byte* a = &answerJson.Pin())
            fixed (byte* v = &valuesJson.Pin())
            {
                found = Native.Result(Native.AskPlannerAdvise(_handle, answerJson.At(a), valuesJson.At(v), &length));
            }
        }

        return JsonSerializer.Deserialize(found, CoreJson.Default.IReadOnlyListAskAdvice)!;
    }

    /// <inheritdoc/>
    public void Dispose()
    {
        _handle.Dispose();
        _gate.Dispose();
    }
}
