namespace Gil;

/// <summary>What a layers or coarse selection chose: the two layers' replays and, when asked for, the replay's decision
/// records.</summary>
/// <param name="Thresholds">The replays of the two memory layers.</param>
/// <param name="Decisions">The replay's decision records, when they were asked for.</param>
public sealed record LayerSelection(LayerThresholds Thresholds, IReadOnlyList<DecisionRecord>? Decisions);

/// <summary>The replays of an attribute's two memory layers, in the order an answer consults them.</summary>
/// <param name="Key">For the key threshold, replayed on every lookup.</param>
/// <param name="Memory">For the memory threshold, replayed on the lookups the chosen key threshold would not have
/// answered.</param>
public sealed record LayerThresholds(ThresholdReplay Key, ThresholdReplay Memory);

/// <summary>What replaying settled records found for one memory layer of an attribute.</summary>
/// <param name="Chosen">The lowest threshold down to which every band of answers met the target precision; null when none
/// does.</param>
/// <param name="MostPrecise">The highest threshold that gathers at least the minimum number of answers; null when no
/// score does.</param>
/// <param name="Lookups">How many lookups the replay made.</param>
/// <param name="Candidates">How many of them found a candidate at any score.</param>
/// <param name="Keys">For the key layer of an attribute that takes one value, the state under each key that backed a best
/// value, most lookups first, then by key in scalar order.</param>
public sealed record ThresholdReplay(
    ThresholdChoice? Chosen,
    ThresholdChoice? MostPrecise,
    int Lookups,
    int Candidates,
    IReadOnlyList<KeyState>? Keys);

/// <summary>A threshold chosen for one memory layer of an attribute, and how it did on the replay it was chosen on. A
/// checked value counts as one ÷ its draw probability lookups, so <see cref="Precision"/> and <see cref="AnswerRate"/>
/// are weighted, while <see cref="Answered"/> and <see cref="Lookups"/> count the lookups made.</summary>
/// <param name="Threshold">The lowest score down to which every band of answers met the target precision.</param>
/// <param name="Precision">The share of answers at or above the threshold that matched the settled value.</param>
/// <param name="AnswerRate">The share of lookups answered at or above the threshold.</param>
/// <param name="Answered">How many lookups were answered.</param>
/// <param name="Lookups">How many lookups the replay made.</param>
/// <param name="AnsweredWeight">The answers with their weights.</param>
/// <param name="EffectiveAnswered">How many independent answers the weighted precision is worth.</param>
/// <param name="SimilarityFloor">For the similar record layer only; always null for the key layer.</param>
/// <param name="Promise">The precision to expect of the next answers: that of the latest records' answers at or above the
/// threshold, lowered to the bottom of its Wilson score interval at one standard error.</param>
/// <param name="PromiseAnswered">How many independent answers <see cref="Promise"/> is worth.</param>
public sealed record ThresholdChoice(
    double Threshold,
    double Precision,
    double AnswerRate,
    int Answered,
    int Lookups,
    double AnsweredWeight,
    double EffectiveAnswered,
    double? SimilarityFloor,
    double Promise,
    double PromiseAnswered);

/// <summary>How the key layer did under one key: the lookups whose best value this key backed most strongly, and how the
/// chosen threshold treated them.</summary>
/// <param name="Key">The key, as an answer's evidence names it — <c>attribute: value</c>.</param>
/// <param name="Lookups">How many lookups this key backed the best value of.</param>
/// <param name="Answered">How many of them the chosen threshold answered; 0 when none was chosen.</param>
/// <param name="Precision">The share of those answers that matched, weighted; null when none was answered.</param>
/// <param name="Answering">Whether the chosen threshold answered this key's latest lookup.</param>
/// <param name="AnsweredWeight">The answers with their weights.</param>
/// <param name="EffectiveAnswered">How many independent answers <see cref="Precision"/> is worth.</param>
public sealed record KeyState(
    string Key,
    int Lookups,
    int Answered,
    double? Precision,
    bool Answering,
    double AnsweredWeight,
    double EffectiveAnswered);

/// <summary>The replays a single-valued attribute's typed key thresholds are chosen on, one per length typed, shortest
/// first.</summary>
/// <param name="ByLength">The replay for one, two, … characters typed.</param>
public sealed record TypedKeyThresholds(IReadOnlyList<ThresholdReplay> ByLength)
{
    /// <summary>The thresholds to set as the attribute's <see cref="Attribute.TypedKeyThresholds"/>: each replay's chosen
    /// one, null where none was.</summary>
    public IReadOnlyList<double?> Thresholds() => [.. ByLength.Select(r => r.Chosen?.Threshold)];
}

/// <summary>The attributes that should key a decided attribute in the key memory, chosen by replaying its key layer, and
/// the replays the choice was made from.</summary>
/// <param name="KeyFields">The attributes to name, in the order they were added; null when no narrower set gained more
/// than every attribute the attribute rests on — leave its key attributes unset.</param>
/// <param name="Chosen">The key layer's replay with the choice.</param>
/// <param name="AllFields">The key layer's replay keyed by every attribute the attribute rests on.</param>
/// <param name="Trials">Every set tried: each attribute it rests on alone, in the schema's order, then each set the greedy
/// additions tried.</param>
/// <param name="Typed">When typing was counted, the typed thresholds that go with the choice.</param>
public sealed record KeyFieldsChoice(
    IReadOnlyList<string>? KeyFields,
    ThresholdReplay Chosen,
    ThresholdReplay AllFields,
    IReadOnlyList<KeyFieldsTrial> Trials,
    TypedKeyThresholds? Typed);

/// <summary>One set of attributes tried for a decided attribute's key attributes, and how its key layer replayed.</summary>
/// <param name="KeyFields">The attributes that keyed the decided attribute in this replay; empty for every attribute it
/// rests on.</param>
/// <param name="Replay">The key layer's replay with them.</param>
/// <param name="Typed">When typing was counted, the typed thresholds with these attributes and their key threshold.</param>
public sealed record KeyFieldsTrial(IReadOnlyList<string> KeyFields, ThresholdReplay Replay, TypedKeyThresholds? Typed);
