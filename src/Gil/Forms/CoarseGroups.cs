namespace Gil.Forms;

/// <summary>
/// A field's coarse level (<see cref="FieldDefinition.Coarse"/>) as the two memory layers back it, shared by suggestions and
/// by the replays its thresholds are chosen on.
/// </summary>
internal static class CoarseGroups
{
    /// <summary>
    /// The class the values under the document's keys back most: each value's score — never below 0 — added to its
    /// class's. The score is that sum, on the key layer's scale, and the evidence the key backing its best value. A tie goes
    /// to the class of the better-ranked value. Null when no value is under the keys.
    /// </summary>
    /// <param name="keyed">The values under the keys with their scores and evidence, best first.</param>
    /// <param name="level">The field's coarse level.</param>
    public static (string Class, double Score, string? Evidence)? Keys(IEnumerable<(string Value, double Score, string? Evidence)> keyed, CoarseLevel level)
    {
        var groups = new List<(string Class, double Score, string? Evidence)>();
        foreach (var (value, score, evidence) in keyed)
        {
            var named = level.Of(value);
            var i = groups.FindIndex(g => string.Equals(g.Class, named, StringComparison.Ordinal));
            if (i < 0)
            {
                groups.Add((named, Math.Max(score, 0), evidence));
            }
            else
            {
                groups[i] = (named, groups[i].Score + Math.Max(score, 0), groups[i].Evidence);
            }
        }

        return groups.Count == 0 ? null : groups.OrderByDescending(g => g.Score).First(); // stable: a tie keeps the better-ranked
    }

    /// <summary>
    /// The class the nearest <paramref name="votes"/> documents vote for, as <see cref="SimilarVote.Decide"/> votes for a
    /// value: each with its similarity, the score the vote's margin over classes, or with a single voter its similarity.
    /// <c>Nearest</c> is the nearest document that voted for the class — its similarity is what
    /// <see cref="CoarseLevel.MemorySimilarityFloor"/> is compared with when more than one voted (<c>Voted</c>). Null when
    /// there are no documents.
    /// </summary>
    /// <param name="nearest">Documents whose value lies in the field's domain, most similar first.</param>
    /// <param name="votes">How many of them vote (<see cref="FieldDefinition.SimilarDocumentVotes"/>); at least 1.</param>
    /// <param name="level">The field's coarse level.</param>
    public static (string Class, double Score, MemoryMatch Nearest, bool Voted)? Vote(IReadOnlyList<MemoryMatch> nearest, int votes, CoarseLevel level)
    {
        if (nearest.Count == 0)
        {
            return null;
        }

        var voters = nearest.Take(votes).ToList();
        if (voters.Count == 1)
        {
            return (level.Of(voters[0].Answer), voters[0].Similarity, voters[0], false);
        }

        var tally = new List<(string Class, MemoryMatch First, double Weight)>();
        foreach (var match in voters)
        {
            var named = level.Of(match.Answer);
            var weight = Math.Max(match.Similarity, 0);
            var i = tally.FindIndex(t => string.Equals(t.Class, named, StringComparison.Ordinal));
            if (i < 0)
            {
                tally.Add((named, match, weight));
            }
            else
            {
                tally[i] = (named, tally[i].First, tally[i].Weight + weight);
            }
        }

        // Stable: classes enter the tally in order of their nearest voter, so a tie keeps the nearer one first.
        var ranked = tally.OrderByDescending(t => t.Weight).ToList();
        var total = ranked.Sum(t => t.Weight);
        var runnerUp = ranked.Count > 1 ? ranked[1].Weight : 0;
        return (ranked[0].Class, total > 0 ? (ranked[0].Weight - runnerUp) / total : 0, ranked[0].First, true);
    }

    /// <summary>
    /// The coarse candidate for a suggestion no layer answered: the keys' class when it reaches the level's key threshold,
    /// else the vote's when it reaches the memory threshold and its nearest voter the floor, else — as a guess — the keys'
    /// class, or the vote's when the keys back none.
    /// </summary>
    public static FieldCandidate? Candidate(
        CoarseLevel level,
        (string Class, double Score, string? Evidence)? keys,
        (string Class, double Score, MemoryMatch Nearest, bool Voted)? vote)
    {
        if (keys is { } k && k.Score >= level.KeyThreshold)
        {
            return new FieldCandidate(k.Class, k.Score, FieldSource.SettledFieldMemory, k.Evidence, Trusted: true);
        }

        if (vote is { } v && Trusted(level, v.Score, v.Nearest.Similarity, v.Voted))
        {
            return new FieldCandidate(v.Class, v.Score, FieldSource.SimilarDocument, v.Nearest.Source, Trusted: true);
        }

        return keys is { } guess
            ? new FieldCandidate(guess.Class, guess.Score, FieldSource.SettledFieldMemory, guess.Evidence, Trusted: false)
            : vote is { } voted ? new FieldCandidate(voted.Class, voted.Score, FieldSource.SimilarDocument, voted.Nearest.Source, Trusted: false) : null;
    }

    /// <summary>Whether the vote's class meets the level's memory threshold and, with more than one voter, its floor.</summary>
    public static bool Trusted(CoarseLevel level, double score, double nearest, bool voted) =>
        score >= level.MemoryThreshold && !(voted && level.MemorySimilarityFloor is { } floor && nearest < floor);
}
