using Gil;

namespace SupportedSurface;

/// <summary>
/// A consumer of the decision core end to end — a schema, records filled and settled, answers with and without an
/// external judgement, a reopened record, thresholds chosen by replay and kept as a tuning, usage counts — written against
/// the supported surface only. The project suppresses no experimental diagnostic.
/// </summary>
public static class FormsWalkthrough
{
    public static async Task<int> RunAsync()
    {
        using var schema = new Schema("ticket",
        [
            new AttributeDefinition("reporter", AttributeRole.Observed) { UseAsEvidence = false },
            new AttributeDefinition("component", AttributeRole.Observed),
            new AttributeDefinition("summary", AttributeRole.Observed),
            new AttributeDefinition("team", AttributeRole.Decided) { MemoryThreshold = 0.5, KeyThreshold = 0.6 },
            new AttributeDefinition("severity", AttributeRole.Decided) { Candidates = ["low", "medium", "high"], Policy = AttributePolicy.ConfirmRequired },
        ]);

        // Filling records puts each change into the decider's memories as it happens.
        using var decider = new Decider();
        var at = DateTimeOffset.UnixEpoch;
        List<Record> saved = [];
        foreach (var (id, component, owner) in new[] { ("t/1", "vpn", "network"), ("t/2", "vpn", "network"), ("t/3", "billing", "finance") })
        {
            at = at.AddMinutes(1);
            using var filling = OpenRecord.Open(schema, id, at);
            filling.Observe(decider, "component", component);
            filling.Observe(decider, "summary", $"{component} trouble");
            filling.Settle(decider, "team", SettlementKind.Enter, at, value: owner);
            saved.Add(filling.Snapshot());
        }

        // At start-up, a decider is filled from the saved records instead.
        using var restarted = new Decider();
        using (var records = SettledRecords.From(saved))
        {
            restarted.Put(schema, records);
        }

        using var record = OpenRecord.Open(schema, "t/4", at.AddMinutes(1));
        record.Observe(restarted, "component", "vpn");
        var answers = record.Observe(restarted, "summary", "vpn drops every ten minutes");
        var team = answers.Single(a => a.Attribute == "team");
        var answeredByKey = team.Answered && team.Source == Source.SettledFieldMemory;
        record.Settle(restarted, "team", answeredByKey ? SettlementKind.Fill : SettlementKind.Enter, at.AddMinutes(1),
            value: answeredByKey ? team.Candidates[0].Value : "network");
        using var reopened = OpenRecord.Reopen(schema, record.Snapshot());

        // An attribute neither memory has evidence for may be judged outside the core.
        using var unknown = OpenRecord.Open(schema, "t/5", at.AddMinutes(2));
        unknown.Observe(restarted, "component", "badge reader");
        var judged = await unknown.AnswerAsync(restarted, new FixedJudge("security"));

        var onScreen = restarted.Answer(schema, "t/6", new Dictionary<string, string> { ["component"] = "billing" });
        var whileTyping = reopened.AnswerPartial(restarted, "severity", "hi");

        // Thresholds chosen by replaying the saved records in settlement order, kept as a tuning between runs.
        using var replayed = SettledRecords.From(saved);
        var options = new SelectionOptions(TargetPrecision: 0.9, MinimumAnswered: 30)
        {
            Counting = [SettlementKind.Accept, SettlementKind.Correct, SettlementKind.Enter],
        };
        var layers = Replay.SelectLayers(schema, "team", replayed, options);
        var narrowed = Replay.SelectKeyFields(schema, "team", replayed, options with { TypedLongest = 2 });
        var concentrations = Replay.SelectKeyConcentrations(schema, "team", replayed);
        using var tuned = new Schema(schema.Name, schema.Attributes.Select(a => a.Name != "team" ? a : a with
        {
            KeyConcentrations = concentrations,
            KeyThreshold = layers.Thresholds.Key.Chosen?.Threshold,
            MemoryThreshold = layers.Thresholds.Memory.Chosen?.Threshold,
            MemoryFloor = layers.Thresholds.Memory.Chosen?.SimilarityFloor,
            KeyFields = narrowed.KeyFields,
            TypedKeyThresholds = narrowed.Typed?.Thresholds,
        }));
        var kept = Tuning.Capture(tuned, saved.Count, options.TargetPrecision).Promise("team", layers.Thresholds).ToJson();
        var tuning = Tuning.FromJson(kept);
        using var applied = tuning.Stale(schema, saved.Count) is null ? tuning.Apply(schema) : null;

        // A coarse level: the class of a coded value, offered when no layer answers the value itself.
        using var coded = new Schema("ticket-coded",
        [
            new AttributeDefinition("component", AttributeRole.Observed),
            new AttributeDefinition("team", AttributeRole.Decided) { Coarse = new CoarseLevel { Prefix = 1 } },
        ]);
        var coarse = Replay.SelectCoarse(coded, "team", replayed, options);

        var usage = Usage.Count([new UsageDecision("team", team.Answered ? team.Candidates[0].Value : null, "network", answeredByKey ? SettlementKind.Fill : SettlementKind.Enter)]);

        return saved.Count + judged.Count + onScreen.Count + (whileTyping is null ? 0 : 1) + (applied is null ? 0 : 1)
            + (coarse.Thresholds.Memory.Chosen is null ? 0 : 1) + usage.Count;
    }

    /// <summary>A judge that always ranks one value — a model, a rule engine or a service takes its place.</summary>
    private sealed class FixedJudge(string value) : IExternalJudge
    {
        public ValueTask<IReadOnlyList<ExternalCandidate>> JudgeAsync(
            AttributeDefinition attribute, IReadOnlyList<ObservedValue> evidence, CancellationToken cancellationToken) =>
            ValueTask.FromResult<IReadOnlyList<ExternalCandidate>>([new ExternalCandidate(value, 0.8, Trusted: true)]);
    }
}
