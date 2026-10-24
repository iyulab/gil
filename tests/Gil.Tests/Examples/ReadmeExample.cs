// The code of README.md's code blocks, compiled here so that the README breaks together with the API.
// ReadmeTests checks that every line of the README's code blocks appears here, in the same order.
using System.Text.Json.Nodes;
using Gil;
using Gil.Fallback;
using Gil.Habits;
using Gil.Judge;
using Gil.Llm;
using Gil.Memory;
using Gil.Tree;
using Gil.Telemetry;
using Gil.Traverse;

namespace Gil.Tests.Examples;

internal static class ReadmeExample
{
    // The decision core (README "Usage"). The external judge is the tree judgment path's resolver and task, below.
    public static async Task DecisionCoreAsync(Resolver resolver, TaskDefinition task)
    {
        IReadOnlyList<Record> savedRecords = [];
        using var schema = new Schema("ticket",
        [
            new AttributeDefinition("reporter", AttributeRole.Observed) { UseAsEvidence = false },
            new AttributeDefinition("component", AttributeRole.Observed),
            new AttributeDefinition("summary", AttributeRole.Observed),
            new AttributeDefinition("team", AttributeRole.Decided) { KeyThreshold = 0.6, MemoryThreshold = 0.5 },
            new AttributeDefinition("severity", AttributeRole.Decided) { Candidates = ["low", "medium", "high"], Policy = AttributePolicy.ConfirmRequired },
        ]);

        using var decider = new Decider();
        using var saved = SettledRecords.From(savedRecords);   // the records settled so far, in settlement order
        decider.Put(schema, saved);

        using var ticket = OpenRecord.Open(schema, "t/4", DateTimeOffset.UtcNow);
        ticket.Observe(decider, "component", "vpn");
        var answers = ticket.Observe(decider, "summary", "vpn drops every ten minutes");
        var team = answers.Single(a => a.Attribute == "team");
        if (team.Answered && team.Policy == AttributePolicy.Suggest)
        {
            ticket.Settle(decider, "team", SettlementKind.Fill, DateTimeOffset.UtcNow, value: team.Candidates[0].Value);
        }

        Record snapshot = ticket.Snapshot();   // keep it; OpenRecord.Reopen(schema, snapshot) carries on from it

        var judge = new ResolverJudge(resolver, new Dictionary<string, TaskDefinition> { ["team"] = task });
        var judged = await ticket.AnswerAsync(decider, judge);

        var options = new SelectionOptions(TargetPrecision: 0.9, MinimumAnswered: 30);
        var keyFields = Replay.SelectKeyFields(schema, "team", saved, options);
        using var keyed = new Schema(schema.Name, schema.Attributes.Select(a => a.Name != "team" ? a : a with
        {
            KeyFields = keyFields.KeyFields,
        }));
        var layers = Replay.SelectLayers(keyed, "team", saved, options);
        using var tuned = keyed.WithLayers("team", layers.Thresholds);
        var tuning = Tuning.Capture(tuned, saved.Count, options.TargetPrecision).Promise("team", layers.Thresholds);
        File.WriteAllText("ticket.tuning.json", tuning.ToJson());

        var kept = Tuning.FromJson(File.ReadAllText("ticket.tuning.json"));
        using var current = kept.Stale(schema, saved.Count) is null ? kept.Apply(schema) : null;

        var shown = team.Answered ? team.Candidates[0].Value : null;
        var promise = team.Answered ? tuning.PromiseOf("team", team.Source) : null;
        var counts = Usage.Count([new UsageDecision("team", shown, "network", SettlementKind.Accept, Promise: promise)]);

        var pooled = Replay.SelectLayers(schema, "team", saved, options with { TargetPrecision = 0.8, Rule = ThresholdRule.Pooled });
        var unprompted = Replay.SelectLayers(schema, "team", saved, options with { Counting = [SettlementKind.Enter] });

        ticket.Settle(decider, "team", SettlementKind.Check, DateTimeOffset.UtcNow, value: "network", probability: 0.05);
        var checkedLayers = Replay.SelectLayers(schema, "team", saved, options with { Counting = [SettlementKind.Enter, SettlementKind.Check] });

        var concentrations = Replay.SelectKeyConcentrations(schema, "team", saved);

        var panel = new CoarseLevel(new Dictionary<string, string> { ["network"] = "infrastructure", ["payments"] = "finance" });
        using var grouped = new Schema(schema.Name, schema.Attributes.Select(a => a.Name != "team" ? a : a with { Coarse = panel }));
        var coarse = Replay.SelectCoarse(grouped, "team", saved, options);

        var typing = Replay.SelectKeyFields(schema, "team", saved, options with { TypedLongest = 3 });
        var typedThresholds = typing.Typed?.Thresholds;   // TypedKeyThresholds, beside KeyFields and KeyThreshold
        var whileTyping = ticket.AnswerPartial(decider, "team", "ne");
    }

    // The tree judgment path (README "The tree judgment path").
    public static async Task TreeJudgmentPathAsync()
    {
        using var store = new SqliteTelemetryStore("gil.sqlite");
        using var chat = IronHiveChatModel.OpenAICompatible(new OpenAICompatibleOptions
        {
            BaseUrl = new Uri("http://localhost:8080/"),
            ApiKey = "",
            Model = "my-model",
            // Sent with every call. A server whose chat template reasons by default must be told not to: otherwise a
            // one-token judgment returns no label, and generation and slot filling spend their budget on reasoning.
            ExtraBody = new JsonObject { ["chat_template_kwargs"] = new JsonObject { ["enable_thinking"] = false } },
        });

        // Calls are priced in the unit you report (GPU milliseconds or dollars), with coefficients fitted for your server.
        var recorder = new CallRecorder(chat, new EnergyModel(Fixed: 120, PerFreshPromptToken: 0.7, PerCachedToken: 0, PerOutputToken: 15), store);

        using var embedder = IronHiveEmbeddingModel.OpenAICompatible(new OpenAICompatibleOptions
        {
            BaseUrl = new Uri("http://localhost:8081/"),
            ApiKey = "",
            Model = "my-embedding-model",
        });
        // Memory is a cache in front of the tree: if it fails, requests go on as misses (the trace keeps the error), and
        // the breaker stops a dead endpoint from adding its retries to every request.
        var remembered = new EmbeddingMemory(new EmbeddingRecorder(embedder, new EnergyModel(Fixed: 0, PerFreshPromptToken: 0.05, PerCachedToken: 0, PerOutputToken: 0), store));
        var memory = new CircuitBreakingMemory(remembered, cooldown: TimeSpan.FromSeconds(30));

        var resolver = new Resolver(
            new GreedyTraverser(new SingleTokenJudge(recorder)),
            new FallbackGenerator(recorder),
            new SlotFiller(recorder),
            sink: store,
            memory: memory,
            statistics: store,
            shadowEvidence: store);

        var tree = TreeYaml.Load("support.yaml").Root;
        // The last argument is the language support.yaml is written in: every prompt the task sends is worded in it.
        var task = new TaskDefinition("support", new TreeAnswerContract(tree), tree, new TaskPolicy
        {
            // No defaults: a judgment's probability is not a calibrated accuracy, and the right threshold depends on the task.
            Thresholds = new Thresholds(PerLayer: [0.7], Leaf: 0.9),
            FallbackScope = FallbackScope.Path,
            // Similarity scales differ between embedding models, so this has no default either; without it memory is off.
            MemoryThreshold = 0.9,
        }, PromptLanguage.English);

        var result = await resolver.ResolveAsync(task, "I lost my card");
        Console.WriteLine($"{result.Mode}: {result.Output}");
        await resolver.FeedbackAsync(task, result.TraceId, correct: true);

        using var lexical = new LexicalMemory();
        await MemoryReplay.From(store.FeedbackHistory("support")).ApplyAsync(lexical, "support", traceId: "startup");
        var offline = new Resolver(lexical, store);
        var bare = TreeYaml.Parse("id: root").Root;   // the tree is not used without models
        var answered = await offline.ResolveAsync(new TaskDefinition("support", new TextContract(), bare,
            new TaskPolicy { Thresholds = new([1.0], 1.0), MemoryThreshold = 0.5 }, PromptLanguage.English), "I lost my card");

        await remembered.RebuildAsync(task.Name, store.FeedbackHistory(task.Name), traceId: "startup");
        await MemoryReplay.From(store.FeedbackHistory(task.Name)).ApplyAsync(memory, task.Name, traceId: "rebuild");

        var proposer = new RepeatedOutputProposer(
            new PromotionPolicy(MinSupport: 3),
            store.JudgeEnergyByNode(task.Name),
            JudgeCostModel.Fit(store.JudgeCostSamples(task.Name)));
        var review = Promotion.Review(tree, proposer.Propose(tree, store.PromotionCandidates(task.Name)));
        File.WriteAllText("support.proposed.yaml", review.After);
        store.RecordPromotionRound(task.Name, atIndex: store.Usage(task.Name).Total, review.Applied);
        store.RecordReview(task.Name, ReviewKind.Promotion, review.Proposals[0].Habit.Id, ReviewDecision.Rejected, "same as billing-refund");

        var fitted = EnergyModel.Fit(store.ServerTimeSamples("my-model"));

        var stats = store.Stats(task.Name, window: 100, pricing: fitted, tree: tree);
    }
}
