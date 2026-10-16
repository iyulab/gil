using Gil;
using Gil.Forms;
using Gil.Memory;

namespace SupportedSurface;

/// <summary>
/// A form consumer end to end — field memory, document memory, suggestions, settlement, threshold selection — written
/// against the supported surface only. The project suppresses no experimental diagnostic.
/// </summary>
public static class FormsWalkthrough
{
    public static async Task<int> RunAsync()
    {
        var form = new FormDefinition("ticket",
        [
            new FieldDefinition("reporter", FieldRole.Observed) { UseAsEvidence = false },
            new FieldDefinition("component", FieldRole.Observed),
            new FieldDefinition("summary", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.5, KeyThreshold = 0.6 },
            new FieldDefinition("severity", FieldRole.Judged) { Candidates = ["low", "medium", "high"], Policy = FieldPolicy.ConfirmRequired },
        ], PromptLanguage.English);

        var forms = new FormResolver(new FieldMemory(), new LexicalMemory(), similarDocumentCount: 3);

        List<SettledDocument> saved = [];
        foreach (var (id, component, owner) in new[] { ("t/1", "vpn", "network"), ("t/2", "vpn", "network"), ("t/3", "billing", "finance") })
        {
            var filling = forms.Open(form, id);
            await filling.ObserveAsync("component", component);
            await filling.ObserveAsync("summary", $"{component} trouble");
            await filling.SettleAsync("team", Settlement.Enter(owner));
            saved.Add(filling.Snapshot());
        }

        await forms.RebuildAsync(form, saved);

        var session = forms.Open(form, "t/4");
        await session.ObserveAsync("component", "vpn");
        var suggestions = await session.ObserveAsync("summary", "vpn drops every ten minutes");
        var team = suggestions.Single(s => s.Field == "team");
        var answeredByKey = team.Answered && team.Source == FieldSource.SettledFieldMemory;
        await session.SettleAsync("team", answeredByKey ? Settlement.Fill(team.Candidates[0].Value) : Settlement.Enter("network"));
        var reopened = forms.Open(form, session.Snapshot());

        var onScreen = await forms.SuggestAsync(form, "t/5", new Dictionary<string, string> { ["component"] = "billing" });
        var whileTyping = await forms.SuggestAsync(form, "t/5", "team", new Dictionary<string, string> { ["component"] = "billing" }, typed: "fi");

        var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30, counting: [SettlementKind.Accept, SettlementKind.Correct, SettlementKind.Enter]);
        var narrowed = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30);
        var typed = ThresholdSelection.SelectTypedKeyThresholds(new FieldMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30);
        _ = form.Field("team") with
        {
            KeyThreshold = layers.Key.Chosen?.Threshold,
            MemoryThreshold = layers.Memory.Chosen?.Threshold,
            MemorySimilarityFloor = layers.Memory.Chosen?.SimilarityFloor,
            KeyFields = narrowed.KeyFields,
            TypedKeyThresholds = typed.Thresholds,
        };

        // Without a document memory: the key layer alone, its coarse level chosen with no memory to replay.
        var keysOnly = new FormResolver(new FieldMemory(), documentMemory: null);
        await keysOnly.RebuildAsync(form, saved);
        var coded = new FormDefinition("ticket-coded",
        [
            new FieldDefinition("component", FieldRole.Observed),
            new FieldDefinition("team", FieldRole.Judged) { Coarse = new CoarseLevel(1) },
        ], PromptLanguage.English);
        var coarse = await ThresholdSelection.SelectCoarseAsync(new FieldMemory(), memory: null, coded, "team", saved, targetPrecision: 0.9, minimumAnswered: 30);

        return saved.Count + (reopened is null ? 0 : 1) + onScreen.Count + (whileTyping is null ? 0 : 1) + (coarse.Memory.Chosen is null ? 0 : 1);
    }
}
