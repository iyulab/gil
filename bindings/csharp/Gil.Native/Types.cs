namespace Gil.Native;

/// <summary>Which layer offered a candidate.</summary>
public enum Layer : byte
{
    /// <summary>No layer.</summary>
    None = 0,

    /// <summary>The settled key memory.</summary>
    SettledFieldMemory = 1,

    /// <summary>A similar settled record.</summary>
    SimilarDocument = 2,

    /// <summary>An external judgement the host offered back.</summary>
    Model = 3,
}

/// <summary>How an attribute's answers are offered.</summary>
public enum AnswerPolicy : byte
{
    /// <summary>Answer when a layer is confident enough.</summary>
    Suggest = 0,

    /// <summary>Answer, but never fill on the host's own.</summary>
    ConfirmRequired = 1,

    /// <summary>Never answer.</summary>
    Off = 2,
}

/// <summary>How a decided attribute was settled; the names are the core's.</summary>
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

/// <summary>A value offered for an attribute: its score on its layer's scale, the layer, and whether the layer trusts
/// it.</summary>
public readonly record struct Candidate(string Value, double Score, Layer Layer, bool Trusted);

/// <summary>The answer for one attribute while a person types into it: the layer of the first candidate, how it is
/// offered, whether it is answered (its first candidate trusted), whether it wants an external judgement, the candidates
/// best first, and the coarse class offered instead when there is one. What backs each candidate is in the JSON
/// answers.</summary>
public sealed record PartialAnswer(
    Layer Layer,
    AnswerPolicy Policy,
    bool Answered,
    bool NeedsExternal,
    IReadOnlyList<Candidate> Candidates,
    Candidate? Coarse);

/// <summary>A value an external judgement ranked: its probability on the judgement's own scale and whether it trusts
/// it.</summary>
public readonly record struct ExternalCandidate(string Value, double Probability, bool Trusted);
