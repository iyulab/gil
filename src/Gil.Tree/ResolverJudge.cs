namespace Gil;

/// <summary>
/// An external judge made of the resolver and one task per decided attribute: the attribute's evidence, as
/// <c>name: value</c> lines, is the request, and the resolver's output is the one value it ranks. An attribute with a
/// closed list of values fits a one-layer tree whose leaves are the values. An attribute without a task, and a request the
/// resolver leaves without an output, abstain.
/// </summary>
/// <remarks>
/// The judgement carries no trace id or energy: each request opens and closes its own trace in the resolver's sink, so
/// <see cref="Resolver.FeedbackAsync"/> on that trace credits the tree as it would any request.
/// </remarks>
[Experimental(Stability.Tree, UrlFormat = Stability.Url)]
public sealed class ResolverJudge : IExternalJudge
{
    private readonly Resolver _resolver;
    private readonly IReadOnlyDictionary<string, TaskDefinition> _tasks;

    /// <param name="resolver">The resolver that judges and generates.</param>
    /// <param name="tasks">Attribute name to the task that judges it.</param>
    public ResolverJudge(Resolver resolver, IReadOnlyDictionary<string, TaskDefinition> tasks)
    {
        ArgumentNullException.ThrowIfNull(resolver);
        ArgumentNullException.ThrowIfNull(tasks);
        (_resolver, _tasks) = (resolver, tasks);
    }

    /// <inheritdoc/>
    public async ValueTask<IReadOnlyList<ExternalCandidate>> JudgeAsync(
        AttributeDefinition attribute, IReadOnlyList<ObservedValue> evidence, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(attribute);
        ArgumentNullException.ThrowIfNull(evidence);
        if (!_tasks.TryGetValue(attribute.Name, out var task))
        {
            return [];
        }

        // The request is the evidence as `name: value` lines, in the order the values arrived.
        var request = string.Join('\n', evidence.Select(e => $"{e.Attribute}: {e.Value}"));
        var resolution = await _resolver.ResolveAsync(task, request, cancellationToken: cancellationToken).ConfigureAwait(false);
        return resolution.Output is null ? [] : [new ExternalCandidate(resolution.Output, resolution.Confidence ?? 0, true)];
    }
}
