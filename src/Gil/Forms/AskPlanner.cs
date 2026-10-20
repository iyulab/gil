using System.Diagnostics.CodeAnalysis;

namespace Gil.Forms;

/// <summary>
/// Which observed field to ask a person for first, so that a judged field gets answered: for each field that could be
/// asked, how much asking it has gained per character typed in documents like this one, and whether it is worth asking
/// within a budget of the typing asking every time would take. Experimental.
/// </summary>
/// <remarks>
/// <para>
/// The planner learns from settled documents, one at a time and in the order they were settled. For each field that
/// could be asked and that the document filled, it compares the judged field's suggestion without that field (the
/// moment before a person typed it) with the suggestion with it, and weighs each by what it would have given: 1 for a
/// value answered right, and for a trusted coarse class that holds the settled value, the share of the field's values
/// it rules out (<c>log(V / V_c) / log V</c>, V the field's values and V_c those in the class); nothing otherwise. The
/// difference, counted only where no layer answered the value without the field, is kept with the characters the field
/// took, under what the suggestion without it showed: its coarse class and whether that class was trusted.
/// </para>
/// <para>
/// Advice ranks the open fields by gain per character — the gain kept under the class the document's suggestion shows,
/// with a prior of 0.1 gained over 10 characters — and marks a field to ask where its gain reaches the lowest gain among
/// the documents learned whose typing, counted from the highest gain down, stays within <see cref="Budget"/> of all
/// their typing, and asking so far has spent no more than that share. A field the suggestion already answers is not
/// worth asking. The budget is counted as though every piece of advice was followed: the planner does not know which
/// fields a person was actually asked.
/// </para>
/// <para>
/// Each field's gain is learned with only that field missing. When several fields are open at once, the suggestion is
/// made without all of them, so advice for more than one is an approximation. Not thread-safe.
/// </para>
/// </remarks>
[Experimental(Stability.Asking, UrlFormat = Stability.Url)]
public sealed class AskPlanner
{
    /// <summary>The gain and the characters every class starts from before anything is learned under it.</summary>
    public const double PriorGain = 0.1;

    /// <inheritdoc cref="PriorGain"/>
    public const double PriorCharacters = 10.0;

    private readonly FormDefinition _form;
    private readonly FieldDefinition _field;
    private readonly Dictionary<string, Ledger> _ledgers;
    private readonly HashSet<string> _seen = new(StringComparer.Ordinal);
    private Dictionary<string, int>? _classSizes;

    /// <param name="form">The form.</param>
    /// <param name="field">A judged field that takes one value.</param>
    /// <param name="askable">The observed fields a person could be asked for; at least one.</param>
    /// <param name="budget">The share of the typing asking every field every time would take that asking may spend; above 0, at most 1.</param>
    public AskPlanner(FormDefinition form, string field, IEnumerable<string> askable, double budget)
    {
        ArgumentNullException.ThrowIfNull(form);
        ArgumentNullException.ThrowIfNull(field);
        ArgumentNullException.ThrowIfNull(askable);
        if (!(budget > 0 && budget <= 1))
        {
            throw new ArgumentOutOfRangeException(nameof(budget), budget, "The budget is a share above 0 and at most 1.");
        }

        var definition = form.Field(field);
        if (definition.Role != FieldRole.Judged || definition.Multiple || definition.Policy == FieldPolicy.Off)
        {
            throw new ArgumentException($"'{field}' is not a judged field that takes one value and is suggested.", nameof(field));
        }

        var fields = askable.ToList();
        if (fields.Count == 0)
        {
            throw new ArgumentException("At least one field must be askable.", nameof(askable));
        }

        _ledgers = new Dictionary<string, Ledger>(StringComparer.Ordinal);
        foreach (var name in fields)
        {
            if (form.Field(name).Role != FieldRole.Observed)
            {
                throw new ArgumentException($"'{name}' is not an observed field.", nameof(askable));
            }

            if (!_ledgers.TryAdd(name, new Ledger()))
            {
                throw new ArgumentException($"'{name}' is given twice.", nameof(askable));
            }
        }

        (_form, _field, Askable, Budget) = (form, definition, fields, budget);
    }

    /// <summary>The fields a person could be asked for, as given.</summary>
    public IReadOnlyList<string> Askable { get; }

    /// <summary>The share of the typing asking every time would take that asking may spend.</summary>
    public double Budget { get; }

    /// <summary>
    /// Learns from a settled document, in settlement order. Asks <paramref name="resolver"/> for the judged field's
    /// suggestion with and without each askable field the document filled — stateless suggestions, which never take the
    /// document as evidence for itself, so the document may already be in the resolver's memory. A document without a
    /// value for the judged field teaches nothing.
    /// </summary>
    /// <param name="resolver">
    /// The resolver the advice will be given beside. Its memory may hold this document, but no document settled after it:
    /// a later document would be evidence the moment before this one was typed could not have had.
    /// </param>
    /// <param name="document">The settled document.</param>
    /// <param name="cancellationToken">Cancels the calls made.</param>
    public async Task LearnAsync(FormResolver resolver, SettledDocument document, CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(resolver);
        ArgumentNullException.ThrowIfNull(document);
        if (!document.Values.TryGetValue(_field.Name, out var truth) || string.IsNullOrWhiteSpace(truth))
        {
            return;
        }

        _seen.Add(truth);
        var known = document.Values.Where(v => v.Key != _field.Name).ToDictionary(v => v.Key, v => v.Value, StringComparer.Ordinal);
        FieldSuggestion? after = null;
        foreach (var name in Askable)
        {
            if (!known.TryGetValue(name, out var value) || string.IsNullOrWhiteSpace(value))
            {
                continue;
            }

            var without = known.Where(v => v.Key != name).ToDictionary(v => v.Key, v => v.Value, StringComparer.Ordinal);
            var before = await SuggestAsync(resolver, document, without, cancellationToken).ConfigureAwait(false);
            if (before.Answered)
            {
                continue;
            }

            after ??= await SuggestAsync(resolver, document, known, cancellationToken).ConfigureAwait(false);
            var c = Weight(before, truth);
            var a = Weight(after, truth);
            _ledgers[name].Learn(ClassOf(before), a - c, value.EnumerateRunes().Count(), Budget);
        }
    }

    /// <summary>
    /// Advice for an open document: every askable field it has no value for, by gain per character, highest first (in
    /// the order given where gains tie), each marked whether to ask.
    /// </summary>
    /// <param name="suggestion">The judged field's suggestion for the document as it stands.</param>
    /// <param name="values">The document's values as they stand.</param>
    public IReadOnlyList<AskAdvice> Advise(FieldSuggestion suggestion, IReadOnlyDictionary<string, string> values)
    {
        ArgumentNullException.ThrowIfNull(suggestion);
        ArgumentNullException.ThrowIfNull(values);
        if (suggestion.Field != _field.Name)
        {
            throw new ArgumentException($"The suggestion is for '{suggestion.Field}', not '{_field.Name}'.", nameof(suggestion));
        }

        var key = ClassOf(suggestion);
        return
        [
            .. Askable
                .Where(name => !values.TryGetValue(name, out var value) || string.IsNullOrWhiteSpace(value))
                .Select(name =>
                {
                    var ledger = _ledgers[name];
                    var gain = ledger.Gain(key);
                    var threshold = ledger.Threshold(Budget);
                    return new AskAdvice(name, gain, !suggestion.Answered && ledger.Asks(gain, Budget), threshold);
                })
                .OrderByDescending(a => a.Gain),
        ];
    }

    private Task<FieldSuggestion> SuggestAsync(FormResolver resolver, SettledDocument document, Dictionary<string, string> values, CancellationToken cancellationToken) =>
        resolver.SuggestAsync(_form, document.DocumentId, _field.Name, values, document.Sets, cancellationToken: cancellationToken);

    private static (string? Class, bool Trusted) ClassOf(FieldSuggestion suggestion) =>
        (suggestion.Coarse?.Value, suggestion.Coarse?.Trusted ?? false);

    /// <summary>What a suggestion gives: 1 for a value answered right, 0 for one answered wrong, else the share a trusted class that holds the settled value rules out.</summary>
    private double Weight(FieldSuggestion suggestion, string truth)
    {
        if (suggestion.Answered)
        {
            return suggestion.Candidates[0].Value == truth ? 1 : 0;
        }

        if (suggestion.Coarse is not { Trusted: true } coarse || _field.Coarse is not { } level)
        {
            return 0;
        }

        int values, inClass;
        if (level.Classes is { } table)
        {
            if (!table.TryGetValue(truth, out var named) || named != coarse.Value)
            {
                return 0;
            }

            _classSizes ??= table.Values.CountBy(c => c).ToDictionary(p => p.Key, p => p.Value, StringComparer.Ordinal);
            values = table.Count;
            inClass = _classSizes[named];
        }
        else
        {
            if (level.Of(truth) != coarse.Value)
            {
                return 0;
            }

            values = _seen.Count;
            inClass = _seen.Count(v => level.Of(v) == coarse.Value);
        }

        return values > 1 ? Math.Log((double)values / inClass) / Math.Log(values) : 0;
    }

    /// <summary>One askable field's gains by class and the budget it has spent.</summary>
    internal sealed class Ledger
    {
        private readonly Dictionary<(string? Class, bool Trusted), (double Gain, double Characters)> _byClass = [];

        // Every learned decision's gain per character at the time, with its characters, ascending.
        private readonly List<(double Gain, int Characters)> _history = [];
        private long _seenCharacters;
        private long _spentCharacters;

        public double Gain((string? Class, bool Trusted) key)
        {
            var (gain, characters) = _byClass.GetValueOrDefault(key);
            return (gain + PriorGain) / (characters + PriorCharacters);
        }

        public double? Threshold(double budget)
        {
            if (_history.Count == 0)
            {
                return null;
            }

            var need = budget * _seenCharacters;
            var counted = 0L;
            var threshold = double.PositiveInfinity;
            for (var i = _history.Count - 1; i >= 0; i--)
            {
                counted += _history[i].Characters;
                threshold = _history[i].Gain;
                if (counted >= need)
                {
                    break;
                }
            }

            return threshold;
        }

        public bool Asks(double gain, double budget) =>
            Threshold(budget) is { } threshold && gain >= threshold && _spentCharacters <= budget * _seenCharacters;

        /// <summary>Learns one decision and says whether the advice before it was to ask.</summary>
        public bool Learn((string? Class, bool Trusted) key, double gained, int characters, double budget)
        {
            var gain = Gain(key);
            var asked = Asks(gain, budget);
            var entry = (gain, characters);
            var at = _history.BinarySearch(entry);
            _history.Insert(at < 0 ? ~at : at + 1, entry);
            _seenCharacters += characters;
            _spentCharacters += asked ? characters : 0;
            var (sum, typed) = _byClass.GetValueOrDefault(key);
            _byClass[key] = (sum + gained, typed + characters);
            return asked;
        }
    }
}

/// <summary>Whether to ask a person for one field of an open document.</summary>
/// <param name="Field">The observed field.</param>
/// <param name="Gain">What asking it has gained per character typed, in documents whose suggestion showed the same class.</param>
/// <param name="Ask">Whether it is worth asking within the budget.</param>
/// <param name="Threshold">The gain per character asking must reach now; null before anything is learned.</param>
[Experimental(Stability.Asking, UrlFormat = Stability.Url)]
public sealed record AskAdvice(string Field, double Gain, bool Ask, double? Threshold);
