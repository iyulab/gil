using System.Text.Json.Serialization;

namespace Gil;

/// <summary>Which layer offered a candidate.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<Source>))]
public enum Source : byte
{
    /// <summary>No layer.</summary>
    None = 0,

    /// <summary>The settled key memory.</summary>
    KeyMemory = 1,

    /// <summary>A similar settled record.</summary>
    SimilarRecord = 2,

    /// <summary>A judgement the host made outside the core and offered back.</summary>
    [JsonStringEnumMemberName("Model")]
    External = 3,
}

/// <summary>How a decided attribute was settled; the names are the core's.</summary>
[JsonConverter(typeof(JsonStringEnumConverter<SettlementKind>))]
public enum SettlementKind
{
    /// <summary>The answered value, kept by a person.</summary>
    Accept,

    /// <summary>A value other than the answered one.</summary>
    Correct,

    /// <summary>The answer turned down, no value settled.</summary>
    Reject,

    /// <summary>The attribute's earlier settlement undone.</summary>
    Revert,

    /// <summary>A saved value put back when a record is reopened.</summary>
    Restore,

    /// <summary>The values of an attribute that takes several.</summary>
    Set,

    /// <summary>A value a person settled with no answer in view.</summary>
    Enter,

    /// <summary>The answered value, settled by the host with no person confirming it.</summary>
    Fill,

    /// <summary>A value settled again by someone who did not see the answer, drawn with a probability.</summary>
    Check,

    /// <summary>A value suggested by something other than this decider, kept by a person.</summary>
    AcceptOther,

    /// <summary>A value a person settled in place of one suggested by something other than this decider.</summary>
    CorrectOther,
}

/// <summary>A value offered for an attribute: its score on its layer's scale, the layer, whether the layer trusts it, and
/// what backs it.</summary>
public readonly record struct Candidate(string Value, double Score, Source Source, bool Trusted)
{
    /// <summary>What backs the candidate — none for the attribute's overall frequency and an external judgement, and
    /// none in a <see cref="PartialAnswer"/>, which leaves it out.</summary>
    public Evidence? Evidence { get; init; }
}

/// <summary>The answer for one attribute while a person types into it: the layer of the first candidate, how it is
/// offered, whether it is answered (its first candidate trusted), whether it wants an external judgement, the candidates
/// best first, and the coarse class offered instead when there is one — read without JSON, so it leaves out what backs each
/// candidate, which <see cref="Answer"/> has.</summary>
public sealed record PartialAnswer(
    Source Source,
    AttributePolicy Policy,
    bool Answered,
    bool NeedsExternal,
    IReadOnlyList<Candidate> Candidates,
    Candidate? Coarse);

/// <summary>A value an external judgement ranked: its probability on the judgement's own scale and whether it trusts
/// it.</summary>
public readonly record struct ExternalCandidate(string Value, double Probability, bool Trusted);

/// <summary>How a layers selection chooses an attribute's two thresholds.</summary>
public enum ThresholdRule
{
    /// <summary>Each layer on its own: the lowest threshold down to which every band of its answers meets the target.
    /// </summary>
    EveryBand,

    /// <summary>The two thresholds together, as the pair that answers the most while the precision of all those answers
    /// taken together stays at or above the target at the low end of its interval.</summary>
    Pooled,

    /// <summary>The answers that cost least per decision, reading the target as where answering pays: a wrong answer
    /// costs 1 / (1 − target) values entered with no suggestion. Each layer's best value gets a calibrated chance of
    /// being right, fitted on the latest records' lookups; the more likely layer answers, at or above one chance
    /// threshold. The answers are right at least the target's share of the time on average, not in every band. The
    /// schema answers by the calibration that comes with the thresholds (<see cref="LayerThresholds.Calibration"/>).
    /// </summary>
    [Experimental(Stability.LeastCost, UrlFormat = Stability.Url)]
    LeastCost,
}

/// <summary>What a threshold is chosen for: the precision to keep (in (0, 1]), the fewest answers a threshold needs,
/// the settlement kinds whose lookups count (all when null), the layers' rule, whether to keep the replay's decision
/// records (the layers' and the coarse level's selections), and how many typed characters to count (the key attributes'
/// selection).</summary>
public sealed record SelectionOptions(double TargetPrecision, int MinimumAnswered)
{
    /// <summary>The settlement kinds whose lookups count; all when null.</summary>
    public IReadOnlyList<SettlementKind>? Counting { get; init; }

    /// <summary>How the layers' selection chooses its two thresholds.</summary>
    public ThresholdRule Rule { get; init; }

    /// <summary>Whether the layers' and the coarse level's selections keep the replay's decision records.</summary>
    public bool Decisions { get; init; }

    /// <summary>The key attributes' selection: count typing up to that many characters (0: none).</summary>
    public int TypedLongest { get; init; }

    /// <summary>The most threads one similar-record lookup of the replay is split among; null for as many as the machine
    /// offers, up to eight; 1 keeps the replay on the caller's thread. The choice is the same whatever it is.</summary>
    public int? Threads { get; init; }

    /// <summary>The keeper's only: whether the first records of batches — whose batch had no settled record when they
    /// were answered — have the value layers' thresholds chosen and promised apart from the rest, each on its own
    /// lookups and resting on <see cref="MinimumAnswered"/>. Where a source sends records in batches that are right or
    /// wrong together, a promise taken on every answer holds for the records after a batch's first and flatters the
    /// first.</summary>
    [Experimental(Stability.FirstOfBatch, UrlFormat = Stability.Url)]
    public bool FirstOfBatch { get; init; }

    /// <summary>The keeper's only: whether a record under a new edition of the code lists (<see cref="Record.FormVersion"/>
    /// — new only when a value comes to mean something else) starts the keeper again from nothing: the records settled
    /// before it leave the decider and the replays, the thresholds chosen on them leave the schema, and records still
    /// arriving under an edition left behind are not taken. Carried across a renumbering, a memory answers with codes
    /// whose meaning changed, and a promise chosen on the old edition does not hold on the new. A record that names no
    /// edition is taken as of the current one; the first edition named starts nothing.</summary>
    [Experimental(Stability.FormVersion, UrlFormat = Stability.Url)]
    public bool RestartOnFormVersion { get; init; }

    /// <summary>The layers' selection and the keeper: whether the thresholds chosen carry, value by settled value, how
    /// they answered (<see cref="LayerThresholds.ValueTails"/>) — the keeper's counted on the records its value
    /// thresholds were chosen on. It moves no threshold.</summary>
    [Experimental(Stability.ValueTails, UrlFormat = Stability.Url)]
    public bool ValueTails { get; init; }

    /// <summary>The keeper's only: whether <see cref="Keeper.Stale"/> also says when the answers stopped keeping the
    /// promise (<see cref="Gil.Stale.Realized"/>) — of an attribute's last 100 answers whose rightness came back
    /// (accepted, corrected, or checked against the answer in view), fewer right than the target less 0.15. It needs the
    /// settlement kinds; records settled with none, or filled by the host, leave it silent. Choosing again does not mend
    /// a revision of the code lists; naming the new edition (<see cref="RestartOnFormVersion"/>) does.</summary>
    [Experimental(Stability.WatchRealized, UrlFormat = Stability.Url)]
    public bool WatchRealized { get; init; }
}

