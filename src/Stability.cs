namespace Gil;

/// <summary>
/// Diagnostic ids of the experimental surface. A type marked with one of these may change shape or disappear in any
/// release without a migration note; using it requires suppressing its id. Everything unmarked is the supported
/// surface, whose breaking changes are listed in the changelog with what to do about them.
/// </summary>
internal static class Stability
{
    /// <summary>
    /// The tree judgment path (package <c>Gil.Tree</c>): the resolver, the tree and its YAML, traversal, one-token judging
    /// and its candidates, the prompt language, output contracts, fallback generation.
    /// </summary>
    public const string Tree = "GIL0001";

    /// <summary>Habit promotion and its review, deactivation, differentiation and shadow evidence (package <c>Gil.Tree</c>).</summary>
    public const string Habits = "GIL0002";

    /// <summary>
    /// Recording and pricing (package <c>Gil.Tree</c>): the telemetry sink and its stored records and statistics, the
    /// call recorders, the energy model, the Microsoft.Extensions.AI embedding adapter.
    /// </summary>
    public const string Recording = "GIL0003";

    /// <summary>
    /// The memory of confirmed answers (package <c>Gil.Tree</c>): the memory port and its matches, the lexical and
    /// embedding memories, the circuit breaker.
    /// </summary>
    public const string Memory = "GIL0004";

    /// <summary>Asking advice (package <c>Gil</c>): which observed attribute to ask a person for first, and whether it is worth asking within a budget.</summary>
    public const string Asking = "GIL0005";

    /// <summary>
    /// The first record of a batch (package <c>Gil</c>): thresholds and promises chosen apart for a record whose batch has
    /// no settled record yet, and answering such a record at them.
    /// </summary>
    public const string FirstOfBatch = "GIL0006";

    /// <summary>
    /// A new edition of the code lists (package <c>Gil</c>): a keeper that starts again from nothing when a record arrives
    /// under a new <c>Record.FormVersion</c>.
    /// </summary>
    public const string FormVersion = "GIL0007";

    /// <summary>
    /// The least-cost rule (package <c>Gil</c>): thresholds chosen where answering costs least when a wrong answer costs
    /// 1 / (1 − target) entries, the layers' calibration that comes with them, and answering by it.
    /// </summary>
    public const string LeastCost = "GIL0008";

    /// <summary>
    /// Value tails (package <c>Gil</c>): a layers selection's report, value by settled value, of how the chosen thresholds
    /// answered.
    /// </summary>
    public const string ValueTails = "GIL0009";

    /// <summary>
    /// Watching the realized precision (package <c>Gil</c>): a keeper that says, as a stale reason, when the answers whose
    /// rightness came back stopped keeping the promise.
    /// </summary>
    public const string WatchRealized = "GIL0010";

    /// <summary>
    /// Key combination (package <c>Gil</c>): an attribute whose key layer scores a value by its strongest key alone rather
    /// than by its strengths under every key added up.
    /// </summary>
    public const string KeyCombine = "GIL0011";

    /// <summary>
    /// The unseen mass in the decision core (package <c>Gil</c>): a keeper that says, attribute by attribute, how much of
    /// the next settled values is likely to be values no record settled yet.
    /// </summary>
    public const string UnseenMass = "GIL0012";

    /// <summary>
    /// Similar elements in the decision core (package <c>Gil</c>): the similar record layer answering an attribute that
    /// takes several values, element by element, by the share of the nearest records that hold each.
    /// </summary>
    public const string SimilarElements = "GIL0013";

    /// <summary>
    /// A threshold for a judge outside Gil (package <c>Gil</c>): its threshold, answer rate and promise chosen from its
    /// scores and what people settled, with nothing else of the judge known.
    /// </summary>
    public const string ScoreThreshold = "GIL0014";

    /// <summary>Where the stability of each surface is described.</summary>
    public const string Url = "https://github.com/iyulab/gil/blob/main/README.md#stability";
}
