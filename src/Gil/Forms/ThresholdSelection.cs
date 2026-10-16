using Gil.Memory;

using System.Diagnostics.CodeAnalysis;
namespace Gil.Forms;

/// <summary>A threshold chosen for one memory layer of a field, and how it did on the replay it was chosen on.</summary>
/// <param name="Threshold">The lowest score down to which every band of answers met the target precision.</param>
/// <param name="Precision">The share of answers at or above the threshold that matched the settled value.</param>
/// <param name="AnswerRate">The share of lookups answered at or above the threshold.</param>
/// <param name="Answered">How many lookups were answered.</param>
/// <param name="Lookups">How many lookups the replay made: every document with a settled value, except the first.</param>
/// <remarks>
/// A checked value (<see cref="SettlementKind.Check"/>) counts as one ÷ its draw probability lookups, so
/// <see cref="Precision"/> and <see cref="AnswerRate"/> are weighted, while <see cref="Answered"/> and
/// <see cref="Lookups"/> count the lookups made. With no check they are the same thing.
/// </remarks>
public sealed record ThresholdChoice(double Threshold, double Precision, double AnswerRate, int Answered, int Lookups)
{
    /// <summary>
    /// The answers with their weights: <see cref="Answered"/> when every settlement counts once, and with checks the
    /// decisions they stand for.
    /// </summary>
    public double AnsweredWeight { get; init; } = Answered;

    /// <summary>
    /// How many independent answers the weighted precision is worth — the squared sum of the weights over the sum of their
    /// squares, <see cref="Answered"/> when every settlement counts once. The minimum number of answers a threshold must
    /// rest on is held to this: a few checks drawn with a small probability weigh much but say little.
    /// </summary>
    public double EffectiveAnswered { get; init; } = Answered;

    /// <summary>
    /// For the similar document layer when more than one document votes, the similarity floor to set with the threshold
    /// (<see cref="FieldDefinition.MemorySimilarityFloor"/>): of the answers at or above the threshold, the similarity of the
    /// nearest document voting for the answer that only a hundredth of them fell below, counting the answers more than one
    /// document voted on. Null for the key layer, for a field with a single voter, and when no answer had more than one
    /// voter: a lone voter's score is its similarity, so the threshold already is its floor.
    /// </summary>
    public double? SimilarityFloor { get; init; }

    /// <summary>
    /// The precision to expect of the next answers: that of the answers at or above the threshold among the most recent
    /// <see cref="ThresholdSelection.RecentDocuments"/> documents replayed, weighted as <see cref="Precision"/> is — or
    /// <see cref="Precision"/> itself when those answers are worth fewer than the minimum number the threshold rests on.
    /// <see cref="Precision"/> runs over the whole replay, and a layer that did better long ago keeps it high after the
    /// layer has changed. Like <see cref="Precision"/>, it is only as honest as the lookups the threshold was chosen on: a
    /// similar document threshold chosen on every lookup, where the key layer answers first, promises too much either way.
    /// </summary>
    public double Promise { get; init; } = Precision;

    /// <summary>How many independent answers <see cref="Promise"/> is worth, counted as <see cref="EffectiveAnswered"/> is.</summary>
    public double PromiseAnswered { get; init; } = Answered;
}

/// <summary>
/// How the key layer did under one key, in a replay that chose a key threshold: the lookups whose best value this key backed
/// most strongly, and how the chosen threshold treated them. A consumer that checks some answered decisions reads it to
/// see which keys answer, and how well, before deciding how often to check each.
/// </summary>
/// <param name="Key">The key, as a suggestion's evidence names it — <c>field: value</c>.</param>
/// <param name="Lookups">How many lookups this key backed the best value of.</param>
/// <param name="Answered">How many of them the chosen threshold answered; 0 when none was chosen.</param>
/// <param name="Precision">The share of those answers that matched, weighted as <see cref="ThresholdChoice.Precision"/> is; null when none was answered.</param>
/// <param name="Answering">Whether the chosen threshold answered this key's latest lookup — whether it answers under it now.</param>
public sealed record KeyState(string Key, int Lookups, int Answered, double? Precision, bool Answering)
{
    /// <summary>The answers with their weights, as <see cref="ThresholdChoice.AnsweredWeight"/>.</summary>
    public double AnsweredWeight { get; init; } = Answered;

    /// <summary>How many independent answers <see cref="Precision"/> is worth, as <see cref="ThresholdChoice.EffectiveAnswered"/>.</summary>
    public double EffectiveAnswered { get; init; } = Answered;
}

/// <summary>
/// What replaying settled documents found for one memory layer of a field: the threshold chosen, if any, and — whether or
/// not one was — how close the layer came, so a field that falls short of the target can say by how much.
/// </summary>
/// <param name="Chosen">The lowest threshold down to which every band of answers met the target precision; null when none does — the layer then should not answer this field on its own.</param>
/// <param name="MostPrecise">
/// The most precise threshold among those resting on at least the minimum number of answers — the highest one that
/// gathers that many; null when the layer found fewer candidates than that at any score. Its precision is the best the
/// layer reached, whether or not that met the target — taken together it can meet the target and still not be chosen,
/// when a band within it falls short.
/// </param>
/// <param name="Lookups">How many lookups the replay made for this layer.</param>
/// <param name="Candidates">How many of them found a candidate at any score.</param>
public sealed record ThresholdReplay(ThresholdChoice? Chosen, ThresholdChoice? MostPrecise, int Lookups, int Candidates)
{
    /// <summary>
    /// For the key layer of a field that takes one value, the state under each key that backed a best value, most lookups
    /// first; null for the similar document layer, a coarse level, typing, and a field that takes several values.
    /// </summary>
    public IReadOnlyList<KeyState>? Keys { get; init; }
}

/// <summary>One set of fields tried for a judged field's <see cref="FieldDefinition.KeyFields"/>, and how its key layer replayed.</summary>
/// <param name="KeyFields">The fields that keyed the judged field in this replay.</param>
/// <param name="Replay">The key layer's replay with them, as <see cref="ThresholdSelection.SelectKeyThreshold"/> makes it.</param>
public sealed record KeyFieldsTrial(IReadOnlyList<string> KeyFields, ThresholdReplay Replay)
{
    /// <summary>
    /// When the field is typed into, the thresholds chosen for typing with these fields and their key threshold, as
    /// <see cref="ThresholdSelection.SelectTypedKeyThresholds"/> makes them; null when typing was not counted.
    /// </summary>
    public TypedKeyThresholds? Typed { get; init; }
}

/// <summary>
/// The fields that should key a judged field in the settled field memory (<see cref="FieldDefinition.KeyFields"/>), chosen
/// by replaying its key layer, and the replays the choice was made from.
/// </summary>
/// <param name="KeyFields">
/// The fields to name, in the order they were added; null when no narrower set answered more often than every field the
/// field rests on — leave <see cref="FieldDefinition.KeyFields"/> unset.
/// </param>
/// <param name="Chosen">The key layer's replay with the choice — keyed by every field it rests on when <paramref name="KeyFields"/> is null.</param>
/// <param name="AllFields">The key layer's replay keyed by every field the field rests on, as it stands without <see cref="FieldDefinition.KeyFields"/>.</param>
/// <param name="Trials">Every set tried: each field it rests on alone, in the form's order, then each set the greedy additions tried.</param>
public sealed record KeyFieldsChoice(IReadOnlyList<string>? KeyFields, ThresholdReplay Chosen, ThresholdReplay AllFields, IReadOnlyList<KeyFieldsTrial> Trials)
{
    /// <summary>
    /// When typing was counted, the thresholds for typing chosen with the choice — set them as
    /// <see cref="FieldDefinition.TypedKeyThresholds"/> with <see cref="Chosen"/>'s threshold as the key threshold.
    /// </summary>
    public TypedKeyThresholds? Typed { get; init; }
}

/// <summary>The key thresholds chosen for typing into a field (<see cref="FieldDefinition.TypedKeyThresholds"/>).</summary>
/// <param name="ByLength">
/// The key layer's replay with one character typed, then two, and so on — each over the documents a person types that far,
/// those the thresholds before did not answer rightly.
/// </param>
public sealed record TypedKeyThresholds(IReadOnlyList<ThresholdReplay> ByLength)
{
    /// <summary>The thresholds to set as <see cref="FieldDefinition.TypedKeyThresholds"/>: each replay's chosen one, null where none was.</summary>
    public IReadOnlyList<double?> Thresholds => [.. ByLength.Select(r => r.Chosen?.Threshold)];
}

/// <summary>The replays of a field's two memory layers, in the order the form resolver consults them.</summary>
/// <param name="Key">For <see cref="FieldDefinition.KeyThreshold"/>, replayed on every lookup.</param>
/// <param name="Memory">For <see cref="FieldDefinition.MemoryThreshold"/>, replayed on the lookups the chosen key threshold would not have answered.</param>
public sealed record LayerThresholds(ThresholdReplay Key, ThresholdReplay Memory);

/// <summary>
/// Chooses <see cref="FieldDefinition.MemoryThreshold"/> and <see cref="FieldDefinition.KeyThreshold"/> by replaying settled documents. The right threshold moves as a
/// memory grows — a small memory's nearest document is rarely close, a large one's often is — so a threshold fixed once
/// loses answers it could give; choose it again as the memory grows (for instance whenever it has grown by a tenth).
/// </summary>
public static class ThresholdSelection
{
    /// <summary>
    /// How many of the latest documents replayed <see cref="ThresholdChoice.Promise"/> is taken over — every document in
    /// settlement order counts, whether or not it was asked about. Long enough that a field answered on a few percent of its
    /// documents still has answers to rest on, short enough to leave behind a stretch the layer did better in.
    /// </summary>
    public const int RecentDocuments = 5_000;

    /// <summary>As <see cref="SelectAsync(LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>, counting every settlement.</summary>
    public static Task<ThresholdReplay> SelectAsync(
        LexicalMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectAsync(memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// Replays the documents in the order they were settled: each is looked up in a memory holding only the documents
    /// settled before it, by the evidence lines its last suggestion was made from — the values that had arrived before
    /// the field (<see cref="SettledDocument.Arrival"/>), its nearest documents voting as a suggestion's do
    /// (<see cref="FieldDefinition.SimilarDocumentVotes"/>) — and then remembered with all its values, replacing an earlier
    /// document of the same case, as the form resolver does. Chooses the lowest threshold down to which every band of answers
    /// reaches <paramref name="targetPrecision"/> — precision fitted as a non-decreasing function of the layer's score, so a weak
    /// band is not admitted on the strength of good answers above it — with at least <paramref name="minimumAnswered"/>
    /// answers in all. When no threshold does, <see cref="ThresholdReplay.Chosen"/> is null and memory should not answer
    /// this field on its own; <see cref="ThresholdReplay.MostPrecise"/> still says how close it came. On its own this
    /// is right only for a field without a <see cref="FieldDefinition.KeyThreshold"/>: with one, a similar document answers
    /// only where no key did — choose both with <see cref="SelectLayersAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>.
    /// </summary>
    /// <param name="memory">An empty memory of the kind in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="form">The form.</param>
    /// <param name="field">A judged field of the form.</param>
    /// <param name="documents">Settled documents, in any order. One settled more than once under the same id is asked about each time without its earlier version as evidence, as a suggestion for a saved document passes over it.</param>
    /// <param name="targetPrecision">The share of answers that must match, in (0, 1].</param>
    /// <param name="minimumAnswered">
    /// The fewest answers a precision may rest on. There is no default: a precision from a handful of answers is noise,
    /// and how much noise the field tolerates is the application's call.
    /// </param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    /// <param name="cancellationToken">Cancels the replay.</param>
    public static Task<ThresholdReplay> SelectAsync(
        LexicalMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default) =>
        SimilarAsync(memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);

    /// <summary>As the overload taking <c>counting</c>, counting every settlement. Experimental (GIL0004).</summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<ThresholdReplay> SelectAsync(
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectAsync(memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// As <see cref="SelectAsync(LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>,
    /// replaying a document memory of another kind — an embedding memory, or one of your own. Experimental: a document
    /// memory other than <see cref="LexicalMemory"/> is outside the supported surface.
    /// </summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<ThresholdReplay> SelectAsync(
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default) =>
        SimilarAsync(memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);

    private static async Task<ThresholdReplay> SimilarAsync(
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(memory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);
        NotASet(form, field);

        var (steps, latest) = await ReplayAsync(null, memory, form, field, documents, counting, cancellationToken).ConfigureAwait(false);
        return Similarity(steps.Where(s => s.Looked), latest, form.Field(field), targetPrecision, minimumAnswered);
    }

    /// <summary>As <see cref="SelectLayersAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>, counting every settlement.</summary>
    public static Task<LayerThresholds> SelectLayersAsync(
        FieldMemory fieldMemory,
        LexicalMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectLayersAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// Chooses both thresholds of a field in the order the form resolver consults its layers. A value under a key answers
    /// first, so <see cref="FieldDefinition.KeyThreshold"/> is chosen as <see cref="SelectKeyThreshold"/> does, on every
    /// lookup. A similar document answers only where no key did, so <see cref="FieldDefinition.MemoryThreshold"/> is
    /// chosen on the lookups the chosen key threshold leaves — those are harder than the rest, and a threshold chosen on
    /// all of them promises more precision than the similar document layer then delivers. One replay serves both: each
    /// document is asked about in memories holding only the documents settled before it, then put into both.
    /// </summary>
    /// <param name="fieldMemory">An empty field memory configured as the one in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="memory">An empty memory of the kind in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="form">The form.</param>
    /// <param name="field">A judged field of the form.</param>
    /// <param name="documents">Settled documents, in any order. One settled more than once under the same id is asked about each time without its earlier version as evidence, as a suggestion for a saved document passes over it.</param>
    /// <param name="targetPrecision">The share of answers that must match, in (0, 1], for each layer.</param>
    /// <param name="minimumAnswered">The fewest answers a precision may rest on, for each layer; the application's call.</param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    /// <param name="cancellationToken">Cancels the replay.</param>
    /// <returns>
    /// A replay of each layer. The memory layer's <see cref="ThresholdReplay.Lookups"/>, and the answer rates of its
    /// thresholds, count only the lookups left to it.
    /// </returns>
    public static Task<LayerThresholds> SelectLayersAsync(
        FieldMemory fieldMemory,
        LexicalMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default) =>
        LayersAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);

    /// <summary>As the overload taking <c>counting</c>, counting every settlement. Experimental (GIL0004).</summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<LayerThresholds> SelectLayersAsync(
        FieldMemory fieldMemory,
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectLayersAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// As <see cref="SelectLayersAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>,
    /// replaying a document memory of another kind. Experimental, as for <see cref="SelectAsync(IMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>.
    /// </summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<LayerThresholds> SelectLayersAsync(
        FieldMemory fieldMemory,
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default) =>
        LayersAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);

    private static async Task<LayerThresholds> LayersAsync(
        FieldMemory fieldMemory,
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(fieldMemory);
        ArgumentNullException.ThrowIfNull(memory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);
        NotASet(form, field);

        var (steps, latest) = await ReplayAsync(fieldMemory, memory, form, field, documents, counting, cancellationToken).ConfigureAwait(false);
        var keyed = steps.Where(s => s.KeyLooked).ToList();
        var answers = keyed.Where(s => s.Key is not null).Select(s => new Answer(s.Key!.Value.Score, s.Key!.Value.Matches, s.Weight, s.Order, s.Key!.Value.Key, s.Key!.Value.Caught)).ToList();
        var key = WithKeys(Fit([.. answers.Where(a => !a.Caught)], keyed, latest, targetPrecision, minimumAnswered), answers);
        var left = steps.Where(s => s.Looked && !(key.Chosen is { } chosen && s.Key is { Caught: false } first && first.Score >= chosen.Threshold));
        return new LayerThresholds(key, Similarity(left, latest, form.Field(field), targetPrecision, minimumAnswered));
    }

    /// <summary>As <see cref="SelectCoarseAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>, counting every settlement.</summary>
    public static Task<LayerThresholds> SelectCoarseAsync(
        FieldMemory fieldMemory,
        LexicalMemory? memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectCoarseAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// Chooses the thresholds of a field's coarse level (<see cref="FieldDefinition.Coarse"/>) in the order a suggestion
    /// offers it: on the lookups the field's value layers leave unanswered at the thresholds it has
    /// (<see cref="FieldDefinition.KeyThreshold"/>, <see cref="FieldDefinition.MemoryThreshold"/> with its floor — choose
    /// those first), the class the keys back, as <see cref="SelectKeyThreshold"/> chooses for a value; on the lookups that
    /// leaves, the class the similar documents vote for, as <see cref="SelectAsync(LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/> does, with its similarity floor. A
    /// class is right when it is the settled value's (<see cref="CoarseLevel.Of"/>). Each document is asked about in
    /// memories holding only the documents settled before it, then put into both, as for the value.
    /// </summary>
    /// <param name="fieldMemory">An empty field memory configured as the one in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="memory">
    /// An empty document memory configured as the one in use; the replay fills it. Never the one serving suggestions.
    /// Null for a form resolver without a document memory: then no similar document answers the value or votes for a
    /// class, and only <see cref="CoarseLevel.KeyThreshold"/> is chosen — the memory layer's replay has no lookups.
    /// </param>
    /// <param name="form">The form, with the field's value thresholds set.</param>
    /// <param name="field">A judged field of the form with a coarse level.</param>
    /// <param name="documents">Settled documents, in any order.</param>
    /// <param name="targetPrecision">The share of classes offered that must be right, in (0, 1], for each layer.</param>
    /// <param name="minimumAnswered">The fewest answers a precision may rest on, for each layer; the application's call.</param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    /// <param name="cancellationToken">Cancels the replay.</param>
    /// <returns>
    /// For <see cref="CoarseLevel.KeyThreshold"/>, and for <see cref="CoarseLevel.MemoryThreshold"/> with
    /// <see cref="ThresholdChoice.SimilarityFloor"/> for <see cref="CoarseLevel.MemorySimilarityFloor"/>. Their
    /// <see cref="ThresholdReplay.Lookups"/> count only the lookups left to each.
    /// </returns>
    public static Task<LayerThresholds> SelectCoarseAsync(
        FieldMemory fieldMemory,
        LexicalMemory? memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default) =>
        CoarseAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);

    /// <summary>As the overload taking <c>counting</c>, counting every settlement. Experimental (GIL0004).</summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<LayerThresholds> SelectCoarseAsync(
        FieldMemory fieldMemory,
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        CancellationToken cancellationToken = default) =>
        SelectCoarseAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting: null, cancellationToken);

    /// <summary>
    /// As <see cref="SelectCoarseAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>,
    /// replaying a document memory of another kind. Experimental, as for <see cref="SelectAsync(IMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/>.
    /// </summary>
    [Experimental(Stability.DocumentMemory, UrlFormat = Stability.Url)]
    public static Task<LayerThresholds> SelectCoarseAsync(
        FieldMemory fieldMemory,
        IMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(memory);
        return CoarseAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered, counting, cancellationToken);
    }

    private static async Task<LayerThresholds> CoarseAsync(
        FieldMemory fieldMemory,
        IMemory? memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(fieldMemory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);
        if (form.Field(field).Coarse is null)
        {
            throw new ArgumentException($"'{field}' has no coarse level to choose thresholds for.", nameof(field));
        }

        var (steps, latest) = await ReplayAsync(fieldMemory, memory, form, field, documents, counting, cancellationToken, coarse: true).ConfigureAwait(false);
        var left = steps.Where(s => (s.KeyLooked || s.Looked) && !s.Coarse!.ValueAnswered).ToList();
        var key = Fit([.. left.Where(s => s.Coarse!.Keys is not null).Select(s => new Answer(s.Coarse!.Keys!.Value.Score, s.Coarse!.Keys!.Value.Correct, s.Weight, s.Order))], left, latest, targetPrecision, minimumAnswered);
        var rest = left.Where(s => !(key.Chosen is { } chosen && s.Coarse!.Keys is { } k && k.Score >= chosen.Threshold)).ToList();
        var votes = rest.Where(s => s.Coarse!.Vote is not null).Select(s => (Vote: s.Coarse!.Vote!.Value, s.Weight, s.Order)).ToList();
        var vote = Fit([.. votes.Select(v => new Answer(v.Vote.Score, v.Vote.Correct, v.Weight, v.Order))], rest, latest, targetPrecision, minimumAnswered);
        var voted = votes.Where(v => v.Vote.Voted).Select(v => (v.Vote.Score, v.Vote.Nearest)).ToList();
        ThresholdChoice? Floored(ThresholdChoice? choice) => choice is null ? null : choice with { SimilarityFloor = SimilarityFloor(voted, choice.Threshold) };
        return new LayerThresholds(key, vote with { Chosen = Floored(vote.Chosen), MostPrecise = Floored(vote.MostPrecise) });
    }

    /// <summary>
    /// Chooses <see cref="FieldDefinition.KeyThreshold"/> the same way: replays the documents in the order they were
    /// settled, asking a field memory holding only the documents settled before each for the best-ranked value under the
    /// keys of the values that had arrived before the field (<see cref="SettledDocument.Arrival"/>), then putting the
    /// whole document. Chooses the lowest score down to which every band of the values
    /// asked about reaches <paramref name="targetPrecision"/>, as for similarity, with at least
    /// <paramref name="minimumAnswered"/> of them in all; when no score does, <see cref="ThresholdReplay.Chosen"/> is
    /// null — the field's keys then should not answer on their own. A document whose keys were never seen before is a lookup without an answer. Repeats of
    /// the same documents answering each other well do not lower the score that a weakly backed value needs. The key layer
    /// is consulted first, on every lookup, so this is right on its own; <see cref="SelectLayersAsync(FieldMemory, LexicalMemory, FormDefinition, string, IEnumerable{SettledDocument}, double, int, IReadOnlyCollection{SettlementKind}?, CancellationToken)"/> chooses it the
    /// same way together with the memory threshold.
    /// <para>
    /// A field that takes several values (<see cref="FieldDefinition.Multiple"/>) trusts each value on its own score, so
    /// every value offered under the keys counts, right when it is one of the settled values not yet chosen. A person
    /// picks such values one after another, and those chosen are evidence for the rest, so each document is asked about
    /// once with none of its values chosen, then again after each but the last is chosen — in a fixed order that depends
    /// on the document and its values but not on the order they are listed in. <see cref="ThresholdChoice.Lookups"/> then
    /// counts the values sought — those not yet chosen, summed over the times the document is asked about — and
    /// <see cref="ThresholdChoice.AnswerRate"/> the values answered per value sought.
    /// </para>
    /// </summary>
    /// <param name="memory">An empty field memory configured as the one in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="form">The form.</param>
    /// <param name="field">A judged field of the form.</param>
    /// <param name="documents">Settled documents, in any order. One settled more than once under the same id is asked about each time without its earlier version as evidence, as a suggestion for a saved document passes over it.</param>
    /// <param name="targetPrecision">The share of answers that must match, in (0, 1].</param>
    /// <param name="minimumAnswered">The fewest answers a precision may rest on; the application's call, as for similarity.</param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    public static ThresholdReplay SelectKeyThreshold(
        FieldMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        IReadOnlyCollection<SettlementKind>? counting = null)
    {
        ArgumentNullException.ThrowIfNull(memory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);

        var multiple = form.Field(field).Multiple;
        var matches = new List<Answer>();
        var (lookups, lookupWeight, order) = (0, 0.0, 0);
        var ordered = documents
            .Select(FormResolver.Remembered) // a filled value is neither remembered nor asked about
            .OrderBy(d => d.SettledAt)
            .ThenBy(d => d.DocumentId, StringComparer.Ordinal); // the order the field memory weighs settlements in
        foreach (var document in ordered)
        {
            order++;
            var settledSet = multiple ? PickOrder(document, field) : [];
            string? settled = null;
            if (multiple ? settledSet.Count == 0 : !document.Values.TryGetValue(field, out settled))
            {
                continue;
            }

            memory.Remove(form.Name, document.DocumentId); // a document settled again is not its own evidence
            var weight = Weight(document, field, counting);
            if (memory.Count(form.Name) > 0 && weight > 0)
            {
                var (before, beforeSets) = (Before(form, document, field), BeforeSets(form, document, field));
                if (!multiple)
                {
                    lookups++;
                    lookupWeight += weight;
                    if (memory.First(form, field, before, settled!, beforeSets) is { } first)
                    {
                        matches.Add(new Answer(first.Score, first.Matches, weight, order, first.Key, first.Caught));
                    }
                }
                else
                {
                    for (var chosen = 0; chosen < settledSet.Count; chosen++)
                    {
                        var sets = new Dictionary<string, IReadOnlyList<string>>(beforeSets, StringComparer.Ordinal) { [field] = settledSet[..chosen] };
                        var sought = new HashSet<string>(settledSet[chosen..], StringComparer.Ordinal);
                        lookups += sought.Count;
                        lookupWeight += sought.Count * weight;
                        matches.AddRange(memory.Scored(form, field, before, sets).Select(s => new Answer(s.Score, sought.Contains(s.Value), weight, order)));
                    }
                }
            }

            memory.Put(form, document);
        }

        var replay = Fit([.. matches.Where(m => !m.Caught)], lookups, lookupWeight, order, targetPrecision, minimumAnswered);
        return multiple ? replay : WithKeys(replay, matches);
    }

    /// <summary>
    /// Chooses the key thresholds that hold while a person types into the field
    /// (<see cref="FieldDefinition.TypedKeyThresholds"/>). Typed text narrows the field to the values that begin with it, so
    /// with each character typed the key layer is a different question, with a threshold of its own. A person types only
    /// where the suggestion before did not do — the threshold for one character is worth only what it does on the
    /// documents its <see cref="FieldDefinition.KeyThreshold"/> did not answer rightly, and so on — and a threshold chosen
    /// on every document promises more than it keeps there, as the documents left are the harder ones. So the replay
    /// supposes a person who types the settled value from its start: each document is asked about with none of it typed,
    /// against the field's <see cref="FieldDefinition.KeyThreshold"/> as it stands, then with one character, two, up to
    /// <paramref name="longest"/>, and the threshold for each length is chosen as <see cref="SelectKeyThreshold"/> chooses,
    /// on the documents not yet answered rightly and long enough to type that far. A document a shorter length trusted a
    /// wrong value on is left out of each length whose typed text still leads to that value: a suggestion keeps a value
    /// trusted with fewer characters typed, so the person is shown that same value there, not a new answer. Set
    /// <see cref="FieldDefinition.KeyThreshold"/> first: these thresholds follow it, and must be chosen again when it changes.
    /// </summary>
    /// <param name="memory">An empty field memory configured as the one in use; the replay fills it. Never the one serving suggestions.</param>
    /// <param name="form">The form.</param>
    /// <param name="field">A judged field of the form that takes one value.</param>
    /// <param name="documents">Settled documents, in any order, as for <see cref="SelectKeyThreshold"/>.</param>
    /// <param name="targetPrecision">The share of answers that must match, in (0, 1].</param>
    /// <param name="minimumAnswered">The fewest answers a precision may rest on, for each length.</param>
    /// <param name="longest">The most characters typed a threshold is chosen for.</param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    /// <exception cref="ArgumentException">The field takes several values.</exception>
    public static TypedKeyThresholds SelectTypedKeyThresholds(
        FieldMemory memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        int longest = 3,
        IReadOnlyCollection<SettlementKind>? counting = null)
    {
        ArgumentNullException.ThrowIfNull(memory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);
        ArgumentOutOfRangeException.ThrowIfLessThan(longest, 1);
        var definition = form.Field(field);
        if (definition.Multiple)
        {
            throw new ArgumentException($"Field '{field}' takes several values; typed key thresholds are for a field that takes one.", nameof(field));
        }

        // Every document asked about: what the key layer's best value was with none of the settled value typed, then
        // with each length of it.
        var asked = new List<Asked>();
        var order = 0;
        var ordered = documents
            .Select(FormResolver.Remembered) // a filled value is neither remembered nor asked about
            .OrderBy(d => d.SettledAt)
            .ThenBy(d => d.DocumentId, StringComparer.Ordinal); // the order the field memory weighs settlements in
        foreach (var document in ordered)
        {
            order++;
            if (!document.Values.TryGetValue(field, out var settled))
            {
                continue;
            }

            memory.Remove(form.Name, document.DocumentId); // a document settled again is not its own evidence
            var weight = Weight(document, field, counting);
            if (memory.Count(form.Name) > 0 && weight > 0)
            {
                var (before, beforeSets) = (Before(form, document, field), BeforeSets(form, document, field));
                asked.Add(new Asked(settled, memory.Typed(form, field, before, settled, longest, beforeSets), weight, order));
            }

            memory.Put(form, document);
        }

        // A value trusted with fewer characters typed is one claim, kept while the typed text still leads to it (see
        // FieldMemory.Rank). A document so answered rightly needs no more typing; one answered wrongly by a value that the
        // next characters still lead to is shown that same claim again, not asked anew, so it is not among the documents a
        // longer threshold is chosen on until the typing has left that value behind.
        foreach (var a in asked)
        {
            a.Claim(a.Best[0], definition.KeyThreshold);
        }

        var byLength = new List<ThresholdReplay>();
        for (var length = 1; length <= longest; length++)
        {
            var reaching = asked.Where(a => !a.Right && a.Best.Length > length && !a.Holds(length)).ToList();
            var matches = reaching
                .Where(a => a.Best[length] is not null)
                .Select(a => new Answer(a.Best[length]!.Value.Score, a.Best[length]!.Value.Value == a.Settled, a.Weight, a.Order))
                .ToList();
            var replay = Fit(matches, reaching.Count, reaching.Sum(a => a.Weight), order, targetPrecision, minimumAnswered);
            byLength.Add(replay);
            foreach (var a in reaching)
            {
                a.Claim(a.Best[length], replay.Chosen?.Threshold);
            }
        }

        return new TypedKeyThresholds(byLength);
    }

    /// <summary>
    /// A set field's settled values, once each, in the order a replay supposes they were picked: fixed by the document id
    /// and the value, so it depends neither on the order the values are listed in nor on their ordinal order.
    /// </summary>
    private static List<string> PickOrder(SettledDocument document, string field) =>
        document.Sets.TryGetValue(field, out var values)
            ? [.. values.Distinct(StringComparer.Ordinal).OrderBy(v => Fnv1a($"{document.DocumentId}\u001f{v}")).ThenBy(v => v, StringComparer.Ordinal)]
            : [];

    /// <summary>A stable 64-bit FNV-1a hash of the text's UTF-16 code units — the same on every run and platform.</summary>
    private static ulong Fnv1a(string text)
    {
        var hash = 14695981039346656037UL;
        foreach (var c in text)
        {
            hash = (hash ^ c) * 1099511628211UL;
        }

        return hash;
    }

    /// <summary>
    /// Chooses the fields that should key a judged field in the settled field memory (<see cref="FieldDefinition.KeyFields"/>)
    /// by how often its key layer then answers at <paramref name="targetPrecision"/>. A key that rarely decides the field
    /// still adds to every value's score, so where a few fields decide a field with many values and the rest only blur it,
    /// keying it by those few answers more often at the same precision. Only the key layer narrows: the similar document
    /// layer and a model keep every field the field rests on (<see cref="FieldDefinition.DependsOn"/>), which narrowing
    /// would starve. Replays the key layer as <see cref="SelectKeyThreshold"/> does with each field the field rests on
    /// alone, ranks them by the answers their chosen threshold gives, then adds them in that order while each addition
    /// answers more often. The result names that set only if it answers more often than every such field
    /// together; otherwise <see cref="KeyFieldsChoice.KeyFields"/> is null. It costs one replay per such field and one
    /// per addition tried. Choose the field's thresholds again with the fields chosen: a narrower set changes the scores.
    /// A field that is typed into answers questions with typed text too, and the fields that decide it before typing may
    /// blur it once a prefix has narrowed its values: give <paramref name="typedLongest"/>, and each set is counted by the
    /// answers before typing and while typing up to that many characters together (<see cref="SelectTypedKeyThresholds"/>,
    /// with the set's key threshold) — the typed thresholds for the choice come with it.
    /// </summary>
    /// <param name="createMemory">Makes an empty field memory configured as the one in use, one per replay. Never the one serving suggestions.</param>
    /// <param name="form">The form.</param>
    /// <param name="field">
    /// A judged field of the form; its own <see cref="FieldDefinition.KeyFields"/> is ignored, and its
    /// <see cref="FieldDefinition.DependsOn"/> bounds the fields tried.
    /// </param>
    /// <param name="documents">Settled documents, in any order, as for <see cref="SelectKeyThreshold"/>.</param>
    /// <param name="targetPrecision">The share of answers that must match, in (0, 1].</param>
    /// <param name="minimumAnswered">The fewest answers a precision may rest on; the application's call, as for similarity.</param>
    /// <param name="typedLongest">
    /// For a field typed into, the most characters typed to count answers for (as for
    /// <see cref="SelectTypedKeyThresholds"/>); 0, the default, counts answers before typing only.
    /// </param>
    /// <param name="counting">
    /// The kinds of settlement (<see cref="SettledDocument.Settlements"/>) whose lookups are counted — say only
    /// <see cref="SettlementKind.Enter"/>, the values settled with no suggestion in view. Every document is still replayed
    /// into memory; the others are just not asked about. Null, the default, counts every settlement, including those not
    /// known; given, a field whose settlement is not known is not counted. A value the application filled
    /// (<see cref="SettlementKind.Fill"/>) is never asked about nor remembered, as memory does not learn it.
    /// </param>
    /// <exception cref="ArgumentException">Typing is counted for a field that takes several values.</exception>
    public static KeyFieldsChoice SelectKeyFields(
        Func<FieldMemory> createMemory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        double targetPrecision,
        int minimumAnswered,
        int typedLongest = 0,
        IReadOnlyCollection<SettlementKind>? counting = null)
    {
        ArgumentNullException.ThrowIfNull(createMemory);
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(documents);
        Check(form, field, targetPrecision, minimumAnswered);
        ArgumentOutOfRangeException.ThrowIfNegative(typedLongest);
        if (typedLongest > 0 && form.Field(field).Multiple)
        {
            throw new ArgumentException($"Field '{field}' takes several values; typing is not counted for it.", nameof(typedLongest));
        }

        var saved = documents.ToList();
        var trials = new List<KeyFieldsTrial>();
        KeyFieldsTrial Replay(IReadOnlyList<string>? keyFields)
        {
            var resting = KeyedBy(form, field, keyFields);
            var replay = SelectKeyThreshold(createMemory(), resting, field, saved, targetPrecision, minimumAnswered, counting);
            var typed = typedLongest == 0
                ? null
                : SelectTypedKeyThresholds(
                    createMemory(), With(resting, field, f => f with { KeyThreshold = replay.Chosen?.Threshold }), field, saved, targetPrecision, minimumAnswered, typedLongest, counting);
            return new KeyFieldsTrial(keyFields ?? [], replay) { Typed = typed };
        }

        KeyFieldsTrial Try(IReadOnlyList<string> keyFields)
        {
            var trial = Replay(keyFields);
            trials.Add(trial);
            return trial;
        }

        static double Answers(KeyFieldsTrial trial) => // weighted: with checks, the decisions they stand for
            (trial.Replay.Chosen?.AnsweredWeight ?? 0) + (trial.Typed?.ByLength.Sum(r => r.Chosen?.AnsweredWeight ?? 0) ?? 0);

        var all = Replay(null);
        var evidence = form.Fields.Where(f => form.Supports(f.Name, field)).Select(f => f.Name).ToList();
        var ranked = evidence
            .Select(name => Try([name]))
            .OrderByDescending(Answers) // stable: on a tie, the form's order
            .ToList();
        var best = ranked.FirstOrDefault();
        var everyField = new KeyFieldsChoice(null, all.Replay, all.Replay, trials) { Typed = all.Typed };
        if (best is null || Answers(best) == 0)
        {
            return everyField;
        }

        foreach (var next in ranked.Skip(1))
        {
            if (best.KeyFields.Count + 1 == evidence.Count)
            {
                break; // every field it rests on: the replay without KeyFields
            }

            var trial = Try([.. best.KeyFields, next.KeyFields[0]]);
            if (Answers(trial) <= Answers(best))
            {
                break;
            }

            best = trial;
        }

        return Answers(best) > Answers(all)
            ? new KeyFieldsChoice(best.KeyFields, best.Replay, all.Replay, trials) { Typed = best.Typed }
            : everyField;
    }

    /// <summary>The form with <paramref name="field"/> keyed by <paramref name="keyFields"/> (every field it rests on when null).</summary>
    private static FormDefinition KeyedBy(FormDefinition form, string field, IReadOnlyList<string>? keyFields) =>
        With(form, field, f => f with { KeyFields = keyFields });

    /// <summary>The form with <paramref name="field"/>'s definition changed.</summary>
    private static FormDefinition With(FormDefinition form, string field, Func<FieldDefinition, FieldDefinition> change) =>
        new(form.Name, [.. form.Fields.Select(f => f.Name == field ? change(f) : f)], form.Language);

    /// <summary>One document of a replay: what the field memory and the document memory held before it said about it.</summary>
    private readonly record struct Step(bool KeyLooked, (double Score, bool Matches, string Key, bool Caught)? Key, bool Looked, (double Score, bool Correct, double Nearest, bool Voted)? Match, double Weight, int Order)
    {
        /// <summary>For a coarse replay: the level as the keys and the vote back it, and whether the field's value layers answered.</summary>
        public CoarseStep? Coarse { get; init; }
    }

    private sealed record CoarseStep(bool ValueAnswered, (double Score, bool Correct)? Keys, (double Score, bool Correct, double Nearest, bool Voted)? Vote);

    /// <summary>
    /// One document of a typed replay: its settled value, the key layer's best value with none of it typed and with each
    /// length typed (<see cref="FieldMemory.Typed"/>), and what the thresholds chosen so far claimed for it.
    /// </summary>
    private sealed class Asked(string settled, (double Score, string Value)?[] best, double weight, int order)
    {
        private readonly List<string> _wrong = [];

        public string Settled { get; } = settled;

        /// <summary>How many decisions the document counts as (see <c>Weight</c>).</summary>
        public double Weight { get; } = weight;

        /// <summary>The document's place in settlement order.</summary>
        public int Order { get; } = order;

        public (double Score, string Value)?[] Best { get; } = best;

        /// <summary>Whether a threshold so far trusted the settled value.</summary>
        public bool Right { get; private set; }

        /// <summary>Records the claim a threshold makes on the best value, if its score reaches it.</summary>
        public void Claim((double Score, string Value)? best, double? threshold)
        {
            if (best is not { } b || !(b.Score >= threshold))
            {
                return;
            }

            if (b.Value == Settled)
            {
                Right = true;
            }
            else
            {
                _wrong.Add(b.Value);
            }
        }

        /// <summary>Whether a wrong claim is still led to with <paramref name="length"/> characters of the settled value typed.</summary>
        public bool Holds(int length) => _wrong.Any(v => FieldMemory.Begins(v, Settled[..length]));
    }

    /// <summary>
    /// Replays the documents in the order they were settled, asking each memory about each document before putting it in
    /// — the document memory keeping one document per case, as the form resolver does.
    /// </summary>
    private static async Task<(List<Step> Steps, int Latest)> ReplayAsync(
        FieldMemory? fieldMemory,
        IMemory? memory,
        FormDefinition form,
        string field,
        IEnumerable<SettledDocument> documents,
        IReadOnlyCollection<SettlementKind>? counting,
        CancellationToken cancellationToken,
        bool coarse = false)
    {
        var task = FormResolver.TaskName(form, field);
        var definition = form.Field(field);
        var traceId = Guid.NewGuid().ToString("N"); // the replay's cost is its own
        var steps = new List<Step>();
        var remembered = 0;
        var cases = new Dictionary<string, string>(StringComparer.Ordinal); // case → the document representing it
        var caseOf = new Dictionary<string, string>(StringComparer.Ordinal); // document → the case it represents
        var ordered = documents
            .Select(FormResolver.Remembered) // a filled value is neither remembered nor asked about
            .OrderBy(d => d.SettledAt)
            .ThenBy(d => d.DocumentId, StringComparer.Ordinal); // the form resolver's order: later wins, then the larger id
        var order = 0;
        foreach (var document in ordered)
        {
            order++;
            if (!document.Values.TryGetValue(field, out var settled))
            {
                continue;
            }

            // A document settled again is not its own evidence, as a suggestion for a saved document passes over it.
            fieldMemory?.Remove(form.Name, document.DocumentId);
            if (memory is not null && caseOf.Remove(document.DocumentId, out var own))
            {
                memory.Forget(task, document.DocumentId);
                cases.Remove(own);
                remembered--;
            }

            // Asked with what the field's last suggestion saw; remembered, below, with everything, as the resolver remembers.
            var (before, beforeSets) = (Before(form, document, field), BeforeSets(form, document, field));
            var keyLooked = fieldMemory is not null && fieldMemory.Count(form.Name) > 0;
            var first = keyLooked ? fieldMemory!.First(form, field, before, settled, beforeSets) : null;
            var evidence = FormResolver.Evidence(form, field, document.Values, document.Sets);
            (double, bool, double, bool)? match = null;
            IReadOnlyList<MemoryMatch> admitted = [];
            if (memory is not null && remembered > 0)
            {
                var asked = FormResolver.Evidence(form, field, before, beforeSets);
                var (found, _) = await memory.NearestAsync(task, asked, definition.SimilarDocumentVotes, traceId, cancellationToken).ConfigureAwait(false);
                // As a suggestion does: only documents whose value lies in the field's domain vote, scored as it scores them.
                admitted = [.. found.Where(m => definition.Admits(m.Answer, before, beforeSets))];
                match = SimilarVote.Decide(admitted, definition.SimilarDocumentVotes) is ({ } winner, var score) ? (score, winner.Answer == settled, winner.Similarity, SimilarVote.Voters(admitted, definition.SimilarDocumentVotes) > 1) : null;
            }

            // A settlement not counted is replayed into memory like any other, but not asked about.
            var weight = Weight(document, field, counting);
            var counted = weight > 0;
            var step = new Step(keyLooked && counted, first, remembered > 0 && counted, match, weight, order);
            if (coarse && definition.Coarse is { } level)
            {
                // As a suggestion: the value layers answer at the field's own thresholds; only where they do not is the level offered.
                var valueAnswered = first is { Caught: false } f && f.Score >= definition.KeyThreshold
                    || match is { } m && m.Item1 >= definition.MemoryThreshold && !(m.Item4 && definition.MemorySimilarityFloor is { } floor && m.Item3 < floor);
                var cls = level.Of(settled);
                var keys = keyLooked ? CoarseGroups.Keys(fieldMemory!.KeyedValues(form, field, before, knownSets: beforeSets), level) : null;
                var vote = CoarseGroups.Vote(admitted, definition.SimilarDocumentVotes, level);
                step = step with
                {
                    Coarse = new CoarseStep(
                        valueAnswered,
                        keys is { } k ? (k.Score, k.Class == cls) : null,
                        vote is { } v ? (v.Score, v.Class == cls, v.Nearest.Similarity, v.Voted) : null),
                };
            }

            steps.Add(step);
            fieldMemory?.Put(form, document);
            if (memory is null)
            {
                continue;
            }

            var key = FormResolver.CaseKey(evidence);
            if (cases.TryGetValue(key, out var earlier))
            {
                memory.Forget(task, earlier);
                caseOf.Remove(earlier);
                remembered--;
            }

            cases[key] = document.DocumentId;
            caseOf[document.DocumentId] = key;
            await memory.RememberAsync(task, document.DocumentId, evidence, settled, traceId, cancellationToken).ConfigureAwait(false);
            remembered++;
        }

        return (steps, order);
    }

    /// <summary>
    /// The document's values that had arrived before <paramref name="field"/> was settled — what its last suggestion was
    /// made from. <see cref="SettledDocument.Arrival"/> says how an unknown order and fields it leaves out are read.
    /// </summary>
    internal static IReadOnlyDictionary<string, string> Before(FormDefinition form, SettledDocument document, string field) =>
        Arrived(form, document, field, document.Values);

    /// <summary>The document's sets that had arrived before <paramref name="field"/> was settled, read as <see cref="Before"/> reads values.</summary>
    internal static IReadOnlyDictionary<string, IReadOnlyList<string>> BeforeSets(FormDefinition form, SettledDocument document, string field) =>
        Arrived(form, document, field, document.Sets);

    private static Dictionary<string, T> Arrived<T>(FormDefinition form, SettledDocument document, string field, IReadOnlyDictionary<string, T> values)
    {
        int Rank(string name)
        {
            if (document.Arrival is { } arrival)
            {
                var at = IndexOf(arrival, name);
                return at >= 0 ? at : name == field ? int.MaxValue : -1;
            }

            var index = IndexOf(form.Fields, name); // a value of no field of the form is no evidence anyway
            return index < 0 || form.Fields[index].Role == FieldRole.Observed ? -1 : index;
        }

        var own = Rank(field);
        var arrived = new Dictionary<string, T>(StringComparer.Ordinal);
        foreach (var (name, value) in values)
        {
            if (name != field && Rank(name) < own)
            {
                arrived[name] = value;
            }
        }

        return arrived;
    }

    private static int IndexOf(IReadOnlyList<string> names, string name)
    {
        for (var i = 0; i < names.Count; i++)
        {
            if (names[i] == name)
            {
                return i;
            }
        }

        return -1;
    }

    private static int IndexOf(IReadOnlyList<FieldDefinition> fields, string name)
    {
        for (var i = 0; i < fields.Count; i++)
        {
            if (fields[i].Name == name)
            {
                return i;
            }
        }

        return -1;
    }

    /// <summary>
    /// The similarity choice over the given lookups: every one counts, answered or not. With more than one voter each
    /// threshold carries the similarity floor of the answers it rests on (<see cref="ThresholdChoice.SimilarityFloor"/>).
    /// </summary>
    private static ThresholdReplay Similarity(IEnumerable<Step> looked, int latest, FieldDefinition definition, double targetPrecision, int minimumAnswered)
    {
        var steps = looked.ToList();
        var matches = steps.Where(s => s.Match is not null).Select(s => s.Match!.Value).ToList();
        var replay = Fit([.. steps.Where(s => s.Match is not null).Select(s => new Answer(s.Match!.Value.Score, s.Match!.Value.Correct, s.Weight, s.Order))], steps, latest, targetPrecision, minimumAnswered);
        if (definition.SimilarDocumentVotes == 1)
        {
            return replay;
        }

        // Only answers decided by a vote say how alike its voters were; a lone voter's score is its similarity already.
        var voted = matches.Where(m => m.Voted).Select(m => (m.Score, m.Nearest)).ToList();
        ThresholdChoice? Floored(ThresholdChoice? choice) => choice is null
            ? null
            : choice with { SimilarityFloor = SimilarityFloor(voted, choice.Threshold) };
        return replay with { Chosen = Floored(replay.Chosen), MostPrecise = Floored(replay.MostPrecise) };
    }

    /// <summary>
    /// The share of a threshold's answers whose nearest voter may fall below its similarity floor. In replays where drafts
    /// were reworded away from the settled documents' words, the lowest similarity alone was pulled down by a single odd
    /// answer and let reworded drafts through; a hundredth kept them below the floor, and a twentieth began to cost answers.
    /// </summary>
    private const double FloorShare = 0.01;

    /// <summary>
    /// Of the answers decided by a vote at or above <paramref name="threshold"/>, the nearest voter's similarity that only
    /// <see cref="FloorShare"/> of them fall below; null when there are none.
    /// </summary>
    internal static double? SimilarityFloor(IEnumerable<(double Score, double Nearest)> answers, double threshold)
    {
        var nearest = answers.Where(a => a.Score >= threshold).Select(a => a.Nearest).Order().ToList();
        return nearest.Count == 0 ? null : nearest[(int)(FloorShare * (nearest.Count - 1))];
    }

    /// <summary>The similar document layer does not suggest a field that takes several values, so it has no threshold to choose.</summary>
    /// <summary>
    /// How many decisions the field's settlement in the document counts as: none when its kind is not counted (every kind is
    /// when none are named), one ÷ the draw probability for a check, one otherwise.
    /// </summary>
    private static double Weight(SettledDocument document, string field, IReadOnlyCollection<SettlementKind>? counting)
    {
        var known = document.Settlements.TryGetValue(field, out var kind);
        if (counting is not null && !(known && counting.Contains(kind)))
        {
            return 0;
        }

        return known && kind == SettlementKind.Check && document.Probabilities.TryGetValue(field, out var probability) ? 1 / probability : 1;
    }

    private static void NotASet(FormDefinition form, string field)
    {
        if (form.Field(field).Multiple)
        {
            throw new ArgumentException($"'{field}' takes several values; the similar document layer does not suggest it — choose its key threshold with SelectKeyThreshold.", nameof(field));
        }
    }

    private static void Check(FormDefinition form, string field, double targetPrecision, int minimumAnswered)
    {
        ArgumentOutOfRangeException.ThrowIfLessThanOrEqual(targetPrecision, 0);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(targetPrecision, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(minimumAnswered, 1);
        if (form.Field(field).Role != FieldRole.Judged)
        {
            throw new ArgumentException($"'{field}' is not a judged field.", nameof(field));
        }
    }

    /// <summary>
    /// Fits precision as a non-decreasing function of the score (isotonic regression by pooling adjacent violators), then
    /// walks the fitted blocks from the highest score down while each block's own precision meets the target. The
    /// threshold is the lowest score of the last such block. Checking every block, not only the answers taken together,
    /// keeps a band of weak answers from being admitted on the strength of many good ones above it. The fitted precision
    /// falls from the top block down, so the answers above a block are at their most precise at the first block that
    /// gathers <paramref name="minimumAnswered"/> of them.
    /// </summary>
    private static ThresholdReplay Fit(List<Answer> matches, List<Step> looked, int latest, double targetPrecision, int minimumAnswered) =>
        Fit(matches, looked.Count, looked.Sum(s => s.Weight), latest, targetPrecision, minimumAnswered);

    private static ThresholdReplay Fit(List<Answer> matches, int lookups, double lookupWeight, int latest, double targetPrecision, int minimumAnswered)
    {
        var replay = Fit(matches, lookups, lookupWeight, targetPrecision, minimumAnswered);
        var recent = matches.Where(m => m.Order > latest - RecentDocuments).ToList();
        ThresholdChoice? Promised(ThresholdChoice? choice)
        {
            if (choice is null)
            {
                return null;
            }

            var (weight, correct, squares) = (0.0, 0.0, 0.0);
            foreach (var m in recent.Where(m => m.Score >= choice.Threshold))
            {
                weight += m.Weight;
                correct += m.Correct ? m.Weight : 0;
                squares += m.Weight * m.Weight;
            }

            var effective = weight > 0 ? weight * weight / squares : 0;
            return effective >= minimumAnswered
                ? choice with { Promise = correct / weight, PromiseAnswered = effective }
                : choice with { Promise = choice.Precision, PromiseAnswered = choice.EffectiveAnswered };
        }

        return replay with { Chosen = Promised(replay.Chosen), MostPrecise = Promised(replay.MostPrecise) };
    }

    /// <summary>
    /// The state under each key that backed a best value: its lookups, and what <paramref name="replay"/>'s chosen
    /// threshold made of them. A lookup where a value the field must not miss was settled under the document's keys is
    /// the key's, but never answered.
    /// </summary>
    private static ThresholdReplay WithKeys(ThresholdReplay replay, IEnumerable<Answer> answers)
    {
        var threshold = replay.Chosen?.Threshold;
        bool Answers(Answer a) => !a.Caught && a.Score >= threshold;
        var keys = answers
            .Where(a => a.Key is not null)
            .GroupBy(a => a.Key!, StringComparer.Ordinal)
            .Select(g =>
            {
                var answered = g.Where(a => Answers(a)).ToList();
                var weight = answered.Sum(a => a.Weight);
                var squares = answered.Sum(a => a.Weight * a.Weight);
                return new KeyState(
                    g.Key,
                    g.Count(),
                    answered.Count,
                    weight > 0 ? answered.Where(a => a.Correct).Sum(a => a.Weight) / weight : null,
                    Answers(g.MaxBy(a => a.Order)))
                {
                    AnsweredWeight = weight,
                    EffectiveAnswered = weight > 0 ? weight * weight / squares : 0,
                };
            })
            .OrderByDescending(k => k.Lookups)
            .ThenBy(k => k.Key, StringComparer.Ordinal)
            .ToList();
        return replay with { Keys = keys };
    }

    private static ThresholdReplay Fit(List<Answer> matches, int lookups, double lookupWeight, double targetPrecision, int minimumAnswered)
    {
        var blocks = new List<Block>();
        foreach (var group in matches.GroupBy(m => m.Score).OrderBy(g => g.Key))
        {
            var block = new Block(group.Key, group.Count(), group.Sum(m => m.Weight), group.Where(m => m.Correct).Sum(m => m.Weight), group.Sum(m => m.Weight * m.Weight));
            while (blocks.Count > 0 && blocks[^1].Precision >= block.Precision)
            {
                var below = blocks[^1];
                blocks.RemoveAt(blocks.Count - 1);
                block = new Block(below.Lowest, below.Count + block.Count, below.Weight + block.Weight, below.Correct + block.Correct, below.Squares + block.Squares);
            }

            blocks.Add(block);
        }

        ThresholdChoice? chosen = null;
        ThresholdChoice? mostPrecise = null;
        var meets = true;
        var (count, weight, correct, squares) = (0, 0.0, 0.0, 0.0);
        for (var i = blocks.Count - 1; i >= 0 && (meets || mostPrecise is null); i--)
        {
            count += blocks[i].Count;
            weight += blocks[i].Weight;
            correct += blocks[i].Correct;
            squares += blocks[i].Squares;
            meets &= blocks[i].Precision >= targetPrecision;
            var effective = weight * weight / squares;
            if (effective >= minimumAnswered)
            {
                var threshold = new ThresholdChoice(blocks[i].Lowest, correct / weight, weight / lookupWeight, count, lookups)
                {
                    AnsweredWeight = weight,
                    EffectiveAnswered = effective,
                };
                mostPrecise ??= threshold;
                if (meets)
                {
                    chosen = threshold;
                }
            }
        }

        return new ThresholdReplay(chosen, mostPrecise, lookups, matches.Count);
    }

    /// <summary>
    /// One answer a layer would give at some threshold: its score, whether it matched, how many decisions it counts as, the
    /// place of its document in settlement order (from 1, counting every document), and for the key layer the key that
    /// backed it most strongly, and whether a value the field must not miss held the key layer back from it.
    /// </summary>
    private readonly record struct Answer(double Score, bool Correct, double Weight, int Order, string? Key = null, bool Caught = false);

    /// <summary>A band of answers the fit pooled: its lowest score, how many answers, their weight, the weight of the right ones, and the sum of the squared weights.</summary>
    private readonly record struct Block(double Lowest, int Count, double Weight, double Correct, double Squares)
    {
        public double Precision => Correct / Weight;
    }
}
