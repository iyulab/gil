using System.Text.Json;
using System.Text.Json.Serialization;

namespace Gil.Native;

/// <summary>One attribute's answer: its candidates, best first, the layer of the first, the coarse class offered instead
/// when no layer answered the value and the attribute has a coarse level, how it is offered, and — when neither memory had
/// evidence and nothing is typed — what an external judgement would read.</summary>
/// <param name="Attribute">The attribute answered.</param>
/// <param name="Candidates">The values offered, best first.</param>
/// <param name="Source">The layer of the first candidate.</param>
/// <param name="Coarse">The class offered instead, when no layer answered the value and the attribute has a coarse
/// level.</param>
/// <param name="Policy">How the attribute's answers are offered — a host fills on its own only what is not
/// <see cref="AttributePolicy.ConfirmRequired"/>.</param>
/// <param name="NeedsExternal">Set when neither memory had evidence and nothing is typed: the values an external
/// judgement would read, in the order they arrived. The host may ask its judgement and offer the result with
/// <see cref="OpenRecord.OfferExternal"/>; ignoring it leaves the answer as it is.</param>
/// <param name="Answered">Whether the first candidate is trusted.</param>
public sealed record Answer(
    string Attribute,
    IReadOnlyList<Candidate> Candidates,
    Source Source,
    Candidate? Coarse,
    AttributePolicy Policy,
    IReadOnlyList<ObservedValue>? NeedsExternal,
    bool Answered)
{
    /// <summary>The lookup's decision record, when it was asked for: what the layers found whatever the thresholds, and
    /// what was answered.</summary>
    public DecisionRecord? Decision { get; init; }
}

/// <summary>An attribute's value as an external judgement reads it.</summary>
[JsonConverter(typeof(ObservedValueConverter))]
public readonly record struct ObservedValue(string Attribute, string Value);

/// <summary>What a candidate rests on, for a host to show: the key backing it, or the similar settled record it came
/// from.</summary>
[JsonConverter(typeof(EvidenceConverter))]
public abstract record Evidence
{
    private protected Evidence()
    {
    }
}

/// <summary>Another attribute's value, as the key memory keeps it — normalized and cut to the memory's key length, so it
/// may differ from the value as settled — under which the candidate was settled.</summary>
public sealed record KeyEvidence(string Attribute, string Value) : Evidence
{
    /// <summary><c>attribute: value</c>.</summary>
    public override string ToString() => $"{Attribute}: {Value}";
}

/// <summary>The settled record whose value the candidate is — for a vote, its nearest voter — and how similar it
/// is.</summary>
public sealed record RecordEvidence(string Id, double Similarity) : Evidence
{
    /// <summary>The record's id.</summary>
    public override string ToString() => Id;
}

/// <summary>One lookup of one attribute. On a replay: a settled record asked about before it is remembered, with what it
/// was settled as. On an answer: a record being filled in, with what the layers found and what was answered — whether it
/// was right is known only once the record is settled.</summary>
/// <param name="Record">The record asked about.</param>
/// <param name="Attribute">The attribute asked about.</param>
/// <param name="Order">On a replay, the record's place in settlement order, from 1 — counting records without the
/// attribute too.</param>
/// <param name="Weight">How many lookups the record counts as: 0 when its settlement is not counted, one ÷ the draw
/// probability for a checked value, otherwise 1. An answer counts as 1.</param>
/// <param name="Key">The key layer's best value, when the key memory held records and a known key led to one.</param>
/// <param name="Similar">The similar record layer's vote, when the record memory held records and one in the attribute's
/// domain voted.</param>
/// <param name="Coarse">On a coarse replay, and on an answer for an attribute with a coarse level when nothing is typed,
/// what the coarse level made of the lookup.</param>
/// <param name="Typed">On a typed replay, the key layer's best value with none, one, … characters of the settled value
/// typed — null at a length no value under the known keys begins with.</param>
/// <param name="Answered">What the answer answered; null when no layer trusted its first candidate, and on a
/// replay.</param>
/// <param name="Settled">The settled value — on a replay.</param>
/// <param name="Settlement">How the attribute was settled, where known.</param>
/// <param name="Probability">For a value settled as a check, the probability it was drawn with.</param>
public sealed record DecisionRecord(
    string Record,
    string Attribute,
    long? Order,
    double Weight,
    KeyLook? Key,
    SimilarLook? Similar,
    CoarseLook? Coarse,
    IReadOnlyList<TypedLook?>? Typed,
    AnsweredValue? Answered,
    string? Settled,
    SettlementKind? Settlement,
    double? Probability);

/// <summary>What the key layer found for one lookup, whatever the threshold.</summary>
/// <param name="Strength">The best value's score.</param>
/// <param name="Value">The best value.</param>
/// <param name="Correct">Whether the best value is the settled one — known on a replay only.</param>
/// <param name="Key">The key backing the best value most strongly, as <c>attribute: value</c>.</param>
/// <param name="Caught">Whether a value the attribute must not miss was settled under the known keys — then the key layer
/// does not answer.</param>
public sealed record KeyLook(double Strength, string Value, bool? Correct, string Key, bool Caught);

/// <summary>What the similar record layer's vote came to for one lookup, whatever the threshold.</summary>
/// <param name="Score">The winning value's score.</param>
/// <param name="Value">The winning value.</param>
/// <param name="Record">The settled record the winning value came from — its nearest voter.</param>
/// <param name="Correct">Whether the winning value is the settled one — known on a replay only.</param>
/// <param name="Nearest">The similarity of the nearest record voting for the winning value — what a similarity floor
/// gates on.</param>
/// <param name="Voted">Whether more than one record voted.</param>
public sealed record SimilarLook(double Score, string Value, string Record, bool? Correct, double Nearest, bool Voted);

/// <summary>What an attribute's coarse level made of one lookup, whatever its thresholds — the level is offered only where
/// the value layers did not answer.</summary>
/// <param name="ValueAnswered">Whether the value layers answered at the attribute's thresholds.</param>
/// <param name="Keys">The class the keys back, its summed strength as <see cref="KeyLook.Strength"/>.</param>
/// <param name="Vote">The class the similar records vote for.</param>
public sealed record CoarseLook(bool ValueAnswered, KeyLook? Keys, SimilarLook? Vote);

/// <summary>The key layer's best value with some of the settled value typed — on a typed replay.</summary>
/// <param name="Strength">The best value's score.</param>
/// <param name="Value">The best value that begins with the typed text.</param>
/// <param name="Correct">Whether it is the settled value.</param>
public sealed record TypedLook(double Strength, string Value, bool Correct);

/// <summary>What an answer answered: the first candidate, when a layer trusted it.</summary>
/// <param name="Layer">The layer that answered.</param>
/// <param name="Value">The value answered.</param>
/// <param name="Threshold">The threshold the layer held it to; null while a person types, when the key layer is held to
/// the threshold for that many characters instead.</param>
public sealed record AnsweredValue(Source Layer, string Value, double? Threshold);

/// <summary>An observed value as the core writes it: <c>[attribute, value]</c>.</summary>
internal sealed class ObservedValueConverter : JsonConverter<ObservedValue>
{
    public override ObservedValue Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        Expect(reader.TokenType == JsonTokenType.StartArray, ref reader);
        reader.Read();
        var attribute = reader.GetString()!;
        reader.Read();
        var value = reader.GetString()!;
        reader.Read();
        Expect(reader.TokenType == JsonTokenType.EndArray, ref reader);
        return new ObservedValue(attribute, value);
    }

    public override void Write(Utf8JsonWriter writer, ObservedValue value, JsonSerializerOptions options)
    {
        writer.WriteStartArray();
        writer.WriteStringValue(value.Attribute);
        writer.WriteStringValue(value.Value);
        writer.WriteEndArray();
    }

    private static void Expect(bool holds, ref Utf8JsonReader reader)
    {
        if (!holds)
        {
            throw new JsonException($"an observed value is [attribute, value], not {reader.TokenType}");
        }
    }
}

/// <summary>Evidence as the core writes it: <c>{"key": {"attribute", "value"}}</c> or <c>{"record": {"id",
/// "similarity"}}</c>.</summary>
internal sealed class EvidenceConverter : JsonConverter<Evidence>
{
    public override Evidence Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType != JsonTokenType.StartObject)
        {
            throw new JsonException($"evidence is an object, not {reader.TokenType}");
        }

        reader.Read();
        var tag = reader.GetString();
        reader.Read();
        Evidence evidence = tag switch
        {
            "key" => JsonSerializer.Deserialize(ref reader, CoreJson.Default.KeyEvidence)!,
            "record" => JsonSerializer.Deserialize(ref reader, CoreJson.Default.RecordEvidence)!,
            _ => throw new JsonException($"evidence is a key or a record, not {tag}"),
        };
        reader.Read();
        return evidence;
    }

    public override void Write(Utf8JsonWriter writer, Evidence value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value)
        {
            case KeyEvidence key:
                writer.WritePropertyName("key");
                JsonSerializer.Serialize(writer, key, CoreJson.Default.KeyEvidence);
                break;
            case RecordEvidence record:
                writer.WritePropertyName("record");
                JsonSerializer.Serialize(writer, record, CoreJson.Default.RecordEvidence);
                break;
        }

        writer.WriteEndObject();
    }
}

/// <summary>A judgement made outside the core — a model, a rule engine, a service — for an attribute neither memory had
/// evidence for.</summary>
public interface IExternalJudge
{
    /// <summary>The values <paramref name="attribute"/> may take, best first, ranked from <paramref name="evidence"/>;
    /// empty to abstain.</summary>
    /// <param name="attribute">The decided attribute to judge, as its schema defines it.</param>
    /// <param name="evidence">The values it may rest on, in the order they arrived — a changed value moving to the end.
    /// </param>
    /// <param name="cancellationToken">Cancels the judgement.</param>
    ValueTask<IReadOnlyList<ExternalCandidate>> JudgeAsync(
        Attribute attribute, IReadOnlyList<ObservedValue> evidence, CancellationToken cancellationToken);
}
