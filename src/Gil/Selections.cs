using System.Text.Json.Serialization;

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
/// <param name="Batches">How much the batches the latest records were settled in flatter the promise; null when none of
/// them names its <see cref="Record.Batch"/>.</param>
public sealed record LayerThresholds(ThresholdReplay Key, ThresholdReplay Memory, BatchCheck? Batches = null)
{
    /// <summary>Value by settled value, how the chosen thresholds answered the latest records settled as it; null
    /// unless asked for (<see cref="SelectionOptions.ValueTails"/>).</summary>
    [Experimental(Stability.ValueTails, UrlFormat = Stability.Url)]
    public IReadOnlyList<ValueTail>? ValueTails { get; init; }

    /// <summary>The calibration <see cref="ThresholdRule.LeastCost"/> chose with; null under the other rules. Put on the
    /// schema with the thresholds (<see cref="Schema.WithLayers"/>), it has the more likely layer answer.</summary>
    [Experimental(Stability.LeastCost, UrlFormat = Stability.Url)]
    public Calibration? Calibration { get; init; }
}

/// <summary>What <see cref="ThresholdRule.LeastCost"/> answers with: each layer's best value's calibrated chance of being
/// right, as a step function of its score, and the chance at or above which the more likely of the two layers
/// answers.</summary>
/// <param name="Key">The key layer's steps, by rising score; empty when it had no best value to fit.</param>
/// <param name="Memory">The similar record layer's steps, by rising score; empty when it had no vote to fit.</param>
/// <param name="Threshold">The chance at or above which an answer is given; null when no chance paid on enough
/// answers.</param>
[Experimental(Stability.LeastCost, UrlFormat = Stability.Url)]
public sealed record Calibration(
    IReadOnlyList<CalibrationStep> Key, IReadOnlyList<CalibrationStep> Memory, double? Threshold);

/// <summary>One step of a calibration: from <paramref name="Score"/> up to the next step's, the chance of being right is
/// <paramref name="Chance"/>; below the first step's score, the first step's chance holds.</summary>
[Experimental(Stability.LeastCost, UrlFormat = Stability.Url)]
public readonly record struct CalibrationStep(double Score, double Chance);

/// <summary>How the chosen thresholds of both layers together answered the latest <see cref="Replay.RecentRecords"/>
/// records settled as one value. A precision promised over the whole attribute is an average over its values, and a
/// value the layers keep taking for another can sit far below it while the average holds:
/// <see cref="AnsweredOtherwise"/> is that value's own share. In two forward replays on public collections, the values
/// whose share was 0.20 or more on the first half of the records were mostly still at 0.10 or more on the second (1.00
/// and 0.88 of them, against 0.10 and 0.06 of the other values) — candidates for a value the attribute must not miss.
/// Reported only for values settled on at least <see cref="Replay.ValueTailDecisions"/> of those records, in value
/// order.</summary>
/// <param name="Value">The settled value.</param>
/// <param name="Decisions">How many of the latest records the replay asked about were settled as it.</param>
/// <param name="AnsweredOtherwise">The share of those the thresholds answered with another value.</param>
/// <param name="GivenInstead">The value given in its place most often; null when none was. Ties go to the first in
/// value order.</param>
[Experimental(Stability.ValueTails, UrlFormat = Stability.Url)]
public sealed record ValueTail(string Value, int Decisions, double AnsweredOtherwise, string? GivenInstead = null);

/// <summary>What the batches among the latest <see cref="Replay.RecentRecords"/> records do to a promise. Records
/// settled together — several at once from one source, alike in what they show — can be right or wrong together, and
/// the replay then counts more independent answers than there are. The latest records are split in two
/// <see cref="Replay.BatchSplits"/> times; on each split the thresholds are chosen on one half, by the same rule, and the
/// answers they give on the other half are counted, both ways. Split by batch, no batch is on both sides; split by
/// record, batch mates are. Where <see cref="AcrossRecords"/> is above <see cref="AcrossBatches"/> by more than chance,
/// the batches flatter a promise taken on records they span, and <see cref="AcrossBatches"/> is the closer count of what
/// the next batches will get. Both count the choosing, not the thresholds chosen on all the records: on records whose
/// batches are not right or wrong together, they can sit above what the next records get by more than the promise
/// does. A record that names no batch is a batch of its own. The check moves no threshold.</summary>
/// <param name="AcrossBatches">The share of the answers counted across halves of the batches that matched, weighted;
/// null when no half chose thresholds that answered.</param>
/// <param name="AcrossBatchesAnswered">How many answers <paramref name="AcrossBatches"/> counts, over every split and
/// both ways.</param>
/// <param name="AcrossRecords">The share of the answers counted across halves of the records that matched,
/// weighted.</param>
/// <param name="AcrossRecordsAnswered">How many answers <paramref name="AcrossRecords"/> counts.</param>
/// <param name="Batches">How many batches the latest records came in, a record that names none counting as one.</param>
/// <param name="BatchSize">How many answers a batch holding more than one holds, in the mean that weighs batch sizes as
/// the correlation does (n₀); null when fewer than two batches hold more than one answer.</param>
/// <param name="Correlation">How much the answers of one batch are right or wrong together under the thresholds chosen on
/// all the records — the intraclass correlation ICC(1) of whether an answer matched.</param>
/// <param name="DesignEffect"><c>1 + (BatchSize − 1) × Correlation</c> — how many answers count as one independent
/// answer. Near one, batches flatter nothing; well above it (seven on public device event reports that flattered a
/// promise by 1.6 points), they do.</param>
public sealed record BatchCheck(
    double? AcrossBatches,
    int AcrossBatchesAnswered,
    double? AcrossRecords,
    int AcrossRecordsAnswered,
    int Batches,
    double? BatchSize,
    double? Correlation,
    double? DesignEffect);

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
    /// <summary>The thresholds to set as the attribute's <see cref="AttributeDefinition.TypedKeyThresholds"/>: each replay's chosen
    /// one, null where none was.</summary>
    [JsonIgnore]
    public IReadOnlyList<double?> Thresholds => [.. ByLength.Select(r => r.Chosen?.Threshold)];
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
