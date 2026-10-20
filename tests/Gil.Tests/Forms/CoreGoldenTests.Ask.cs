using System.Text.Json.Nodes;
using Gil.Forms;
using Gil.Memory;

namespace Gil.Tests.Forms;

public sealed partial class CoreGoldenTests
{
    private static readonly string[] Askable = ["product", "channel", "summary"];

    /// <summary>
    /// <see cref="AskPlanner"/> for each single-valued judged field, with the settings the golden chose (ask.jsonl): a
    /// resolver rebuilt from the first records, then for each later record the field's suggestion and the advice with some
    /// askable fields left open — the product every other record, the channel every third, the summary every fifth — before the
    /// record is put, and the planner learning from it after. The budget is 0.3 of the typing.
    /// </summary>
    private static async Task<List<string>> AskAsync(FormDefinition form, IReadOnlyList<SettledDocument> documents)
    {
        var resolver = new FormResolver(new FieldMemory(), new LexicalMemory());
        await resolver.RebuildAsync(form, documents.Take(Chosen).ToList(), Ct);
        var planners = form.Fields
            .Where(f => f.Role == FieldRole.Judged && !f.Multiple)
            .Select(f => (f.Name, Planner: new AskPlanner(form, f.Name, Askable, 0.3)))
            .ToList();
        var lines = new List<string>();
        var i = 0;
        foreach (var document in documents.Skip(Chosen))
        {
            var open = document.Values
                .Where(v => form.Field(v.Key).Role == FieldRole.Observed && !Open(v.Key, i))
                .ToDictionary(StringComparer.Ordinal);
            foreach (var (field, planner) in planners)
            {
                var suggestion = await resolver.SuggestAsync(form, document.DocumentId, field, open, cancellationToken: Ct);
                lines.Add(Line(new JsonObject
                {
                    ["kind"] = "ask",
                    ["doc"] = document.DocumentId,
                    ["field"] = field,
                    ["answered"] = suggestion.Answered,
                    ["coarse"] = suggestion.Coarse is { } k ? new JsonArray(k.Value, k.Trusted) : null,
                    ["advice"] = new JsonArray([.. planner.Advise(suggestion, open).Select(a => (JsonNode)new JsonArray(a.Field, a.Gain, a.Ask, a.Threshold))]),
                }));
            }

            await resolver.RebuildAsync(form, [document], Ct);
            foreach (var (_, planner) in planners)
            {
                await planner.LearnAsync(resolver, document, Ct);
            }

            i++;
        }

        return lines;

        static bool Open(string field, int i) => (field == "product" && i % 2 == 0) || (field == "channel" && i % 3 == 1) || (field == "summary" && i % 5 == 0);
    }
}
