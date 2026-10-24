using System.Collections.ObjectModel;
using System.Text.Json.Serialization;

namespace Gil;

/// <summary>Whether an attribute is read or decided.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<AttributeRole>))]
public enum AttributeRole
{
    /// <summary>A value the host reads in and the decider answers from.</summary>
    [JsonStringEnumMemberName("observed")]
    Observed,

    /// <summary>A value a person settles and the decider answers.</summary>
    [JsonStringEnumMemberName("decided")]
    Decided,
}

/// <summary>How a decided attribute's answers are offered.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<AttributePolicy>))]
public enum AttributePolicy : byte
{
    /// <summary>Answer when a layer is confident enough.</summary>
    [JsonStringEnumMemberName("suggest")]
    Suggest = 0,

    /// <summary>Answer, but never fill on the host's own — for attributes where a wrong value accepted without a look
    /// costs more than typing it.</summary>
    [JsonStringEnumMemberName("confirm_required")]
    ConfirmRequired = 1,

    /// <summary>Never answer — for attributes a model or a memory should not decide, such as an assessment of a
    /// person.</summary>
    [JsonStringEnumMemberName("off")]
    Off = 2,
}

/// <summary>A decided attribute's coarse level, offered when neither layer answers the value: the class a value belongs to
/// — its first <see cref="Prefix"/> characters, or what <see cref="Classes"/> says — and the thresholds at which the class
/// the two layers back is trusted.</summary>
public sealed record CoarseLevel
{
    /// <summary>A level with neither a prefix nor a table yet — set one of them.</summary>
    public CoarseLevel()
    {
    }

    /// <summary>A level whose classes are the first <paramref name="prefix"/> characters of a value.</summary>
    public CoarseLevel(int prefix) => Prefix = prefix;

    /// <summary>A level whose classes a table names: value to class.</summary>
    public CoarseLevel(IReadOnlyDictionary<string, string> classes) => Classes = classes;

    /// <summary>How many leading characters (Unicode scalar values) of a value name its class; 0 for a level made from a
    /// table.</summary>
    public int Prefix { get; init; }

    /// <summary>Value to class, for a level made from a table — at least one entry, no class blank; a value it does not list
    /// is a class of its own.</summary>
    public IReadOnlyDictionary<string, string>? Classes { get; init; }

    /// <summary>The summed key scores of a class's values at or above which the keys' class is trusted; null trusts
    /// none.</summary>
    public double? KeyThreshold { get; init; }

    /// <summary>The vote's margin over classes at or above which the similar records' class is trusted; null trusts
    /// none.</summary>
    public double? MemoryThreshold { get; init; }

    /// <summary>The similarity the nearest record voting for the class must also reach when more than one
    /// voted.</summary>
    public double? MemoryFloor { get; init; }

    /// <summary>The class of <paramref name="value"/>: what the table says, the value itself when the table does not list
    /// it; without a table, its first <see cref="Prefix"/> Unicode scalar values, or all of it when shorter.</summary>
    public string Of(string value)
    {
        ArgumentNullException.ThrowIfNull(value);
        if (Classes is not null)
        {
            return Classes.TryGetValue(value, out var found) ? found : value;
        }

        var end = 0;
        var taken = 0;
        foreach (var rune in value.EnumerateRunes())
        {
            if (taken == Prefix)
            {
                break;
            }

            end += rune.Utf16SequenceLength;
            taken++;
        }

        return value[..end];
    }
}

/// <summary>One attribute of a schema, with the settings that govern how it is answered; a setting left out takes the
/// core's default.</summary>
/// <param name="Name">The attribute's name, unique in its schema.</param>
/// <param name="Role">Whether it is read or decided.</param>
public sealed record AttributeDefinition(string Name, AttributeRole Role)
{
    /// <summary>A set of values rather than one.</summary>
    public bool Multiple { get; init; }

    /// <summary>False keeps the attribute out of every other attribute's evidence.</summary>
    public bool UseAsEvidence { get; init; } = true;

    /// <summary>The attributes its answers may rest on; null is every other attribute that is evidence.</summary>
    public IReadOnlyList<string>? DependsOn { get; init; }

    /// <summary>The attributes that key it in the key memory; null is every attribute it rests on.</summary>
    public IReadOnlyList<string>? KeyFields { get; init; }

    /// <summary>How readily each key attribute's keys are believed; an attribute not named takes 1.</summary>
    public IReadOnlyDictionary<string, double> KeyConcentrations { get; init; } = ReadOnlyDictionary<string, double>.Empty;

    /// <summary>The score the first keyed value must reach to be trusted; null trusts none.</summary>
    public double? KeyThreshold { get; init; }

    /// <summary>How many of the most similar records vote on its value; at least 1.</summary>
    public int SimilarVotes { get; init; } = 10;

    /// <summary>The vote's score at or above which the similar record layer is trusted; null trusts none.</summary>
    public double? MemoryThreshold { get; init; }

    /// <summary>The similarity the nearest record voting for the value must also reach when more than one voted.
    /// </summary>
    public double? MemoryFloor { get; init; }

    /// <summary>Its coarse level, offered when neither layer answers the value.</summary>
    public CoarseLevel? Coarse { get; init; }

    /// <summary>The values it may take; null is any. A value settled outside them is remembered but never offered.
    /// </summary>
    public IReadOnlyList<string>? Candidates { get; init; }

    /// <summary>Another attribute whose value in the same record bounds this one's values — for an attribute that takes
    /// several, the values chosen in it. Once that attribute has a value no layer offers a value outside it; while it is
    /// empty the values are not narrowed.</summary>
    public string? CandidatesFrom { get; init; }

    /// <summary>Values it must not miss: the key layer does not answer for a record whose keys have ever had one.
    /// </summary>
    public IReadOnlyList<string>? MustCatch { get; init; }

    /// <summary>The score the first keyed value that begins with the typed text must reach, for one, two, … characters
    /// typed; null (or a length beyond the list) trusts none.</summary>
    public IReadOnlyList<double?>? TypedKeyThresholds { get; init; }

    /// <summary>How its answers are offered; an open record never answers an attribute that is off.</summary>
    public AttributePolicy Policy { get; init; }
}

/// <summary>A record's values as last settled: the values and sets of its attributes, when it was settled, how each
/// decided attribute was settled where known, the probability each check was drawn with, and the order the current
/// values arrived in.</summary>
/// <param name="Id">The record's id, unique in its schema.</param>
/// <param name="SettledAt">When it was settled.</param>
public sealed record Record(string Id, DateTimeOffset SettledAt)
{
    /// <summary>The value of each attribute that takes one.</summary>
    public IReadOnlyDictionary<string, string> Values { get; init; } = ReadOnlyDictionary<string, string>.Empty;

    /// <summary>The values of each attribute that takes several.</summary>
    public IReadOnlyDictionary<string, IReadOnlyList<string>> Sets { get; init; } =
        ReadOnlyDictionary<string, IReadOnlyList<string>>.Empty;

    /// <summary>How each decided attribute was settled, where known.</summary>
    public IReadOnlyDictionary<string, SettlementKind> Settlements { get; init; } =
        ReadOnlyDictionary<string, SettlementKind>.Empty;

    /// <summary>For an attribute settled as a check, the probability it was drawn with.</summary>
    public IReadOnlyDictionary<string, double> Probabilities { get; init; } = ReadOnlyDictionary<string, double>.Empty;

    /// <summary>The attributes with a value, in the order their current values arrived, oldest first; null when the order
    /// is unknown — the observed values then count as there from the start and the decided ones as settled in the
    /// schema's order.</summary>
    public IReadOnlyList<string>? Arrival { get; init; }

    /// <summary>The batch the record was settled in — records settled together, several at once from one source — when
    /// known; records of one batch name the same. A layers selection counts what the batches do to its promise
    /// (<see cref="LayerThresholds.Batches"/>).</summary>
    public string? Batch { get; init; }

    /// <summary>For each decided attribute settled with a suggestion in view, the value suggested — this decider's answer,
    /// or what the host showed in its place (the settlement says which). For a coarse level, the class shown.</summary>
    public IReadOnlyDictionary<string, string>? Suggested { get; init; }

    /// <summary>For each decided attribute settled with this decider's answer in view, the layer that gave it.</summary>
    public IReadOnlyDictionary<string, Source>? Sources { get; init; }

    /// <summary>For each decided attribute settled with this decider's answer in view, the precision promised for it when
    /// it was shown — what <see cref="Tuning.PromiseOf"/> gave for its layer; each in [0, 1].</summary>
    public IReadOnlyDictionary<string, double>? Promises { get; init; }

    /// <summary>For each decided attribute, the milliseconds from when the person reached it — its suggestion came into
    /// view, or it was reached with nothing in view — to when it was settled.</summary>
    public IReadOnlyDictionary<string, ulong>? SettleMs { get; init; }

    /// <summary>The milliseconds the record was open, from opening it to this settlement.</summary>
    public ulong? OpenMs { get; init; }

    /// <summary>For each decided attribute, who settled it, as an opaque id the host gives — one person, one id; not a
    /// name. One record may be settled by several people, an attribute each.</summary>
    public IReadOnlyDictionary<string, string>? SettledBy { get; init; }

    /// <summary>The version of the form the record was settled on, as the host identifies it — the edition of the code
    /// lists its decided attributes are settled from, which changes when a value comes to mean something else, not when
    /// only a label changes.</summary>
    public string? FormVersion { get; init; }
}
