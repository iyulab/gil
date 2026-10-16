namespace Gil;

/// <summary>
/// Diagnostic ids of the experimental surface. A type marked with one of these may change shape or disappear in any
/// release without a migration note; using it requires suppressing its id. Everything unmarked is the supported
/// surface, whose breaking changes are listed in the changelog with what to do about them.
/// </summary>
internal static class Stability
{
    /// <summary>The tree judgment path: the resolver, the tree and its YAML, traversal, one-token judging, fallback generation.</summary>
    public const string Tree = "GIL0001";

    /// <summary>Habit promotion and its review, deactivation, differentiation and shadow evidence.</summary>
    public const string Habits = "GIL0002";

    /// <summary>Recording and pricing: the telemetry sink and its stored records and statistics, the call recorders, the energy model.</summary>
    public const string Recording = "GIL0003";

    /// <summary>
    /// A document memory other than <c>LexicalMemory</c> on the form path: the embedding memory, the circuit breaker, a
    /// memory of your own. The supported document memory is the lexical one, and the form path takes only it.
    /// </summary>
    public const string DocumentMemory = "GIL0004";

    /// <summary>Where the stability of each surface is described.</summary>
    public const string Url = "https://github.com/iyulab/gilfit/blob/main/README.md#stability";
}
