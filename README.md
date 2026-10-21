# Gil

Suggests the decided attributes of a record — a ticket's team, a report's fault code — from the records already settled,
and says how often a suggestion it trusts is right. It needs no model: it learns from the values settled together and
from the settled records most like the one being filled, and how far to trust each is chosen by replaying the saved
records in the order they were settled. The library also carries an experimental path that answers requests in front
of an LLM from confirmed answers and one-token judgments; see [Stability](#stability).

**Status: early.** The library is being built from a specification whose behaviour was measured first in a
research harness. Published on NuGet as `Gil`, the decision core — native (Rust) inside, the package carrying its
library for each supported runtime — and, for the experimental path in front of an LLM, `Gil.Tree` with
`Gil.Abstractions`, `Gil.IronHive` and `Gil.Sqlite`; the API may still change within 0.x —
[CHANGELOG.md](https://github.com/iyulab/gil/blob/main/CHANGELOG.md) lists each change and what to do about it.

## When to use it

Gil is for a form that is filled in again and again — support tickets, service requests, maintenance reports — where
some attributes are typed and others take one of a set of values that the typed ones mostly decide, and where what a
person finally settled is kept. It suggests those decided attributes one at a time:

- **From values settled together**: a value settled alongside the same value of another attribute, weighted toward the
  more recent.
- **From similar settled records**: the nearest settled records vote, compared by their characters with no model,
  or by meaning through an embedding model you choose.
- **At a precision you choose**: each layer's threshold is chosen by replaying the saved records, so a suggestion it
  trusts carries the precision the replay saw. What falls short is still offered as an untrusted guess.

What was measured, on a public stream of aircraft maintenance reports: letting the replay choose the attributes that key a
fault code with several hundred values made the key layer answer it 1.9 times as often as keying it by every attribute, at
a precision of 0.94 against 0.90; and the code's chapter — its first two characters — was trusted for 42% of the reports
whose code went unanswered at a target of 0.95, and was right for 97% of them. On public NCBI BioSample records, an
[association-rule recommender and Gil are compared](https://github.com/iyulab/gil/blob/main/benchmarks/biosample/README.md)
by how many values each answers at the same target precision.
The precision a replay promises can still be optimistic, most of all where settled records arrive in batches.
Measure it on your own saved records before relying on it.

It is not the right tool for an attribute whose value the other attributes do not decide (free text, a new name), for a form
nobody settles (memory then stays empty), or for choosing between models (a router does that).

## Layout

| Project | Contents |
|---|---|
| `Gil.Abstractions` | The model ports: `IChatModel` and `IEmbeddingModel` with their requests and results |
| `Gil` | The decision core, native (Rust) behind its C ABI: schemas, settled records, deciders, open records and their typed answers, threshold replay and tuning, usage counts, asking advice (`GIL0005`). It needs no other package |
| `Gil.Tree` | Everything experimental (`GIL0001`–`GIL0004`): the tree judgment path — the tree YAML reader/writer, the single-token judge, the greedy traverser, output contracts, the fallback generator, slot filling, the resolver (memory → tree → narrowed fallback → full fallback), habits and their promotion, deactivation and differentiation, shadows — and what it is built on: the memories of confirmed answers, the call recorders and energy model, the telemetry contracts |
| `Gil.IronHive` | Chat and embedding models through any IronHive generator, with ready-made ones for OpenAI-compatible servers (retry rules for busy shared servers, the response kept as received). Optional: implement `IChatModel` and `IEmbeddingModel` yourself and `Gil` needs nothing else |
| `Gil.Sqlite` | The telemetry store: every trace and model call, habit statistics, promotion and review logs in one SQLite file whose layout is the contract below. Optional: `Gil.Tree` takes any `ITelemetrySink`, and a consumer that keeps no telemetry ships no SQLite native library |
| `Gil.Core.Tests` | Tests of the decision core through its .NET API, reproducing the Rust core's goldens |
| `Gil.Tests` | Unit tests of the experimental tracks and the compatibility fixture writer |

The `Gil` package carries the decision core's native library for six runtimes, and the .NET host loads the one for
the platform it runs on:

| Runtime | Built for |
|---|---|
| `linux-x64`, `linux-arm64` | glibc 2.28 or later (the manylinux 2.28 baseline) |
| `osx-x64`, `osx-arm64` | macOS 11 or later |
| `win-x64`, `win-arm64` | Windows, with the C runtime linked in (no Visual C++ redistributable needed) |

On any other runtime — musl-based Linux such as Alpine, 32-bit platforms — the first call throws a
`DllNotFoundException` that names the runtime; build the library from this repository
(`cargo build --release -p gil-ffi`) and put it next to the application.

The telemetry file layout is a contract: analysis tools read it directly, so columns may be added but never
renamed or repurposed. `Writes_the_compatibility_fixture` produces a small synthetic store other
implementations can open to check they read the same layout (set `GIL_COMPAT_FIXTURE` to choose where).

The same contract is checked the other way on every build: judgments, traversal, prompts, the resolver (with and
without memory, including memory failures), shadows, promotion, deactivation, differentiation and tree files are
replayed from synthetic fixtures under `tests/fixtures/conformance`, which the reference implementation produced from a
made-up support task and a deterministic model. Set the matching `GIL_COMPAT_*` variable to replay a recorded run instead.

## Stability

The library has a supported surface and an experimental one.

**Supported** are the decision core (`Gil`, but for asking advice), the chat and embedding model ports
(`Gil.Abstractions`) and their IronHive models. Within 0.x a release may still change them; each such change is listed in the changelog under **Breaking** with what to do.

**Experimental** types carry `[Experimental]`. Using one is a compile error until its id is suppressed, and a marked
type may change shape or be removed in any release without a migration note.

| Id | Covers |
|---|---|
| `GIL0001` | The tree judgment path: `Resolver`, task and tree definitions, `TreeYaml`, traversal, one-token judging (`JudgeCandidate`, `Judgment`), `PromptLanguage`, output contracts and fallback generation, and `ResolverJudge`, the resolver as the decision core's external judge |
| `GIL0002` | Habit promotion: proposals and their review, deactivation, differentiation, shadow evidence |
| `GIL0003` | Recording and pricing: `ITelemetrySink` and `SqliteTelemetryStore`, the call recorders, `EnergyModel`, `EmbeddingGeneratorModel`, statistics, rebuilding a memory from the feedback log |
| `GIL0004` | The memory of confirmed answers: `IMemory`, `MemoryMatch`, `LexicalMemory`, `EmbeddingMemory`, `CircuitBreakingMemory` |
| `GIL0005` | Asking advice in the decision core: `AskPlanner`, `AskAdvice` |

Suppress the ids of what you use, in the project file or around the code:

```xml
<PropertyGroup>
  <NoWarn>$(NoWarn);GIL0001;GIL0003</NoWarn>
</PropertyGroup>
```

The walkthrough under [Usage](#usage) is the tree judgment path: it needs `GIL0001`, `GIL0003` and `GIL0004`, and
`GIL0002` for its habit promotion part.

### The tree judgment path

What the experimental ids cover is the library's first shape: confirmed-answer memory and cheap one-token judgments in
front of an LLM. Repeated requests are answered from memory or from a small tree of single-token judgments; only what
they cannot settle goes to full generation, narrowed to the category the tree already confirmed. Every call is recorded
and priced. Promoting answers that keep coming back into habits is off unless a task turns it on, and behind a review
gate.

It is for a task that keeps receiving requests of the same few kinds — support tickets, service requests,
recurring log messages — where each request needs one answer from a known set of categories and someone can say,
at least some of the time, whether the answer was right. It sits in front of a model you already call:

- **Memory** answers a request that means the same as one whose answer was confirmed. Like a semantic cache, but
  only confirmed answers enter it, and an answer later marked wrong is forgotten.
- **The tree** of one-token judgments settles what memory missed when the categories are clear enough to judge,
  and narrows generation to the confirmed category when they are not.
- **Every call is recorded and priced**, so whether the tree pays for itself is read off the log rather than assumed.

What was measured: wherever requests repeated, memory did most of the work. Behind memory, the tree with narrowed
generation beat full generation — one to three points more accurate at about half the cost — on an English benchmark
with a hosted model. With a mid-size open model on non-English service data it made no difference to accuracy, and
its cost ran from somewhat lower to nearly double, depending on how long the full generation prompt was. Promoting
answers into habits did not pay on the realistic datasets and stays off unless a task turns it on. Measure it on your
own log before relying on it.

It is not the right tool for free conversation (there is no answer to remember or category to judge), for choosing
between models (a router does that), or for a task that never gets feedback (memory then stays empty — see
"Memory lives in the process" below for filling it from answers you already have confirmed).

## Usage

This walkthrough is the tree judgment path, which is experimental — see [Stability](#stability) for the ids to suppress.

```sh
dotnet add package Gil.Tree       # the tree judgment path this walkthrough uses, its memories and recorders
dotnet add package Gil.IronHive   # the models below; skip it if you bring your own IChatModel
dotnet add package Gil.Sqlite     # the telemetry store below; skip it if you keep no telemetry
```

A request goes through memory first, then the tree of one-token judgments, then a fallback narrowed to the
category the tree confirmed, then the full fallback. Every model call is recorded and priced in the telemetry
store, and feedback on an answer is what memory, habit statistics and promotion learn from.

```csharp
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
```

Memory can embed through any Microsoft.Extensions.AI `IEmbeddingGenerator<string, Embedding<float>>` instead:
`new EmbeddingGeneratorModel(generator)` takes the place of the IronHive embedder, with the provider's usage and model
recorded the same way.
That includes a generator running on the same machine (an ONNX model, for example): Gil takes the generator, not
the model files, so which model to ship stays your decision.

Where no model is reachable at all, `LexicalMemory` compares requests by their characters (n-gram TF-IDF), with no
tokenizer and no call, and `new Resolver(memory, sink)` resolves with memory alone — a request memory cannot answer
abstains instead of reaching a model. It is the decision core's similar record memory, run by the native library the
`Gil` package carries, so dispose it when you are done with it. Its similarities are on their own scale: pick the
task's `MemoryThreshold` by replaying your feedback history rather than reusing an embedding memory's value.

```csharp
using var lexical = new LexicalMemory();
await MemoryReplay.From(store.FeedbackHistory("support")).ApplyAsync(lexical, "support", traceId: "startup");
var offline = new Resolver(lexical, store);
var bare = TreeYaml.Parse("id: root").Root;   // the tree is not used without models
var answered = await offline.ResolveAsync(new TaskDefinition("support", new TextContract(), bare,
    new TaskPolicy { Thresholds = new([1.0], 1.0), MemoryThreshold = 0.5 }, PromptLanguage.English), "I lost my card");
```

The tree file names the categories and, under each, the answers a request can get. Only `id` is required; a
field left out takes the value shown in the comment, and the writer leaves out any field equal to it, so a file
written back after a review diffs cleanly against the one a person wrote:

```yaml
id: root
children:
  - id: billing
    description: Charges, invoices, refunds, payment methods   # default: empty — but this is what the judge reads
    options:
      - id: billing-refund
        text: "Refunds reach the original payment method within 5 business days."
        # kind: answer (or template with slots, or procedure with steps) · label: the id · origin: seed
  - id: cards
    description: Lost, stolen or blocked cards
    # label_scheme: inherited from the parent, letters at the root (digits holds fewer candidates)
    options:
      - id: cards-block
        text: "Freeze the card in the app under Cards > Freeze; a replacement ships in 3 days."
```

A node has either children (categories) or options (answers), not both. Loading warns when siblings' descriptions
share distinctive words, since the judge may not tell them apart.

`TreeAnswerContract` makes the fallback pick one of the tree's answers (or "none of these"); it never writes a new
one. A request that fits no answer exactly gets the closest listed one, or none. For answers written case by case,
give the task a `TextContract` instead: the fallback then generates, and answers confirmed by feedback can be
remembered and later proposed as habits.

A task whose answers are free text may gain nothing from judgments; define it with a bare root (`id: root`) and
it goes from memory straight to the fallback without a judgment call.

Every task declares the language its tree is written in, and every prompt the task sends — judgments, the
fallback, slot filling, the contract's instructions and the violations fed back on a retry — is worded in it. There
is no default for the same reason as the thresholds: "none of these" and the tree's candidates must share a language,
or out-of-scope requests stop being rejected. Two languages are built in: `PromptLanguage.Korean`, the wording the
behaviour was measured with (kept as measured), and `PromptLanguage.English`. To change a piece of the wording, start
from one of them, for example `PromptLanguage.English with { NoneOfThese = "Not applicable" }`.

Memory lives in the process. It is an index over the feedback in the log, so a restarted process starts empty
until it is rebuilt from that log — the same call fills it from answers confirmed elsewhere, once they are in the
store:

```csharp
await remembered.RebuildAsync(task.Name, store.FeedbackHistory(task.Name), traceId: "startup");
```

Memory is a port, `IMemory`, so it can live elsewhere — a vector store several instances share, for example. The
contract is three calls: `LookupAsync` returns the task's nearest remembered request (keyed by its trace id, with the
similarity — the task's `MemoryThreshold` decides whether it answers) and the energy the lookup cost; `RememberAsync`
stores a confirmed answer under the request's trace id; `Forget` removes one. The resolver calls them as verdicts
arrive, and a lookup that throws is a miss (or an error, per `MemoryFailure`), so `CircuitBreakingMemory` wraps any of
them. To fill a store from the log, `MemoryReplay` reads the same rule off the feedback history and applies it through
those calls. Tenants belong in the task name — one task per tenant and job — so nothing crosses between them:

```csharp
await MemoryReplay.From(store.FeedbackHistory(task.Name)).ApplyAsync(memory, task.Name, traceId: "rebuild");
```

Promotion never edits the tree by itself. Whoever operates the task runs a round, reviews the result against the
authored YAML and applies what they accept:

```csharp
var proposer = new RepeatedOutputProposer(
    new PromotionPolicy(MinSupport: 3),
    store.JudgeEnergyByNode(task.Name),
    JudgeCostModel.Fit(store.JudgeCostSamples(task.Name)));
var review = Promotion.Review(tree, proposer.Propose(tree, store.PromotionCandidates(task.Name)));
File.WriteAllText("support.proposed.yaml", review.After);
store.RecordPromotionRound(task.Name, atIndex: store.Usage(task.Name).Total, review.Applied);
```

Record the round once you apply it: `atIndex` is how many of the task's requests were resolved before the proposer ran,
and `Applied` holds what the tree took (an empty round is recorded too). A later review or a restart then sees exactly
what was proposed when, instead of running the proposer again over a log that has grown since.

By default a proposal comes only from fallback answers confirmed as correct. When the right answer is one only
your organisation knows, it arrives as a correction instead; set `FromCorrections = true` on the policy to count
corrections too. A round that proposes nothing says so only by an empty list.

`Deactivation.Propose` and `Differentiation.Capacity` / `Differentiation.Anchored` produce the other two review
lists: habits to retire, and nodes to split or categories to add. `Differentiation.UnservedAsync` adds the requests
the task keeps answering "none of these" to, grouped by how similar they are — each group is either a category to
add or out-of-scope input to confirm as such. Record what the reviewer decided on each item, with
the reason when there is one — the log then shows what was accepted and rejected, and what reviewing the task costs:

```csharp
store.RecordReview(task.Name, ReviewKind.Promotion, review.Proposals[0].Habit.Id, ReviewDecision.Rejected, "same as billing-refund");
```

Costs are only as real as the coefficients. A self-hosted server reports how long each call took; once a few hundred
calls are recorded, fit the coefficients to those times and price with them from then on (for an API, use its
published prices instead):

```csharp
var fitted = EnergyModel.Fit(store.ServerTimeSamples("my-model"));
```

Accuracy is reported, not promised. `Stats` reads it off the log — per mode, on the requests that got feedback, with a
95% interval — together with the share of requests habits and memory answered and the cost per request over time
(priced again with the fitted coefficients, so early and late requests compare in one unit). Given the tree, it also
counts misroutes: requests whose confirmed answer lies outside the category the tree sent them to.

```csharp
var stats = store.Stats(task.Name, window: 100, pricing: fitted, tree: tree);
```

Live, the same story goes to your tracing and metrics backend. Gil emits through the BCL under one name,
`"Gil"`: a `gil.resolve` span per request with the task, mode, memory outcome (`off`, `hit`,
`miss`, or `failed` — a lookup that failed and was treated as a miss), energy and confidence, a `gil.call` span under
it per model call with its role, node and tokens, and the metrics `gil.resolutions` (by task, mode and memory
outcome), `gil.resolution.energy`, `gil.resolution.duration` and `gil.call.energy`. With OpenTelemetry, add
`AddSource("Gil")` and `AddMeter("Gil")`. Provider spans and token metrics (`gen_ai.*`) come
from the model client, not from Gil, and nest under `gil.call`.

## Filling forms

A form is a `Schema`: the attributes a host reads in (`Observed`) and those the decider answers (`Decided`), each with
the settings that govern its answers. A `Decider` holds the memories — values settled together, similar settled
records — and an `OpenRecord` is one record while its values arrive: each change answers again and goes into the
decider's memories.

```csharp
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
```

An answer is `Answered` when its first candidate is trusted at the attribute's thresholds; the other candidates are
offered as untrusted guesses, and each says which layer offered it and what backs it (`Evidence`: the key, or the
similar record). A person's decision is settled with the kind that says how it was made — `Accept`, `Correct`,
`Enter`, `Fill`, … — since memory and the replays treat them differently. `decider.Answer(schema, id, values)` answers
from values alone and writes nothing; `AnswerPartial` answers one attribute while a person types into it.

When neither memory has evidence for an attribute, its answer's `NeedsExternal` lists the values a judgement outside the
core would read. `AnswerAsync` asks an `IExternalJudge` — a model, a rule engine, a service — for each of them and puts
what it ranks into the answers. `ResolverJudge` (`Gil.Tree`, `GIL0001`) is the tree judgment path above as one:

```csharp
var judge = new ResolverJudge(resolver, new Dictionary<string, TaskDefinition> { ["team"] = task });
var judged = await ticket.AnswerAsync(decider, judge);
```

How far to trust each layer is chosen by replaying the settled records in settlement order, for the precision you
want. Put the choice on the schema, and keep it as a `Tuning`: at start-up, `Stale` says whether it still stands — the
same core version, the same settings it rests on, about as many records — and `Apply` puts it back.

```csharp
var options = new SelectionOptions(TargetPrecision: 0.9, MinimumAnswered: 30);
var layers = Replay.SelectLayers(schema, "team", saved, options);
var keyFields = Replay.SelectKeyFields(schema, "team", saved, options);
using var tuned = new Schema(schema.Name, schema.Attributes.Select(a => a.Name != "team" ? a : a with
{
    KeyFields = keyFields.KeyFields,
    KeyThreshold = layers.Thresholds.Key.Chosen?.Threshold,
    MemoryThreshold = layers.Thresholds.Memory.Chosen?.Threshold,
    MemoryFloor = layers.Thresholds.Memory.Chosen?.SimilarityFloor,
}));
var tuning = Tuning.Capture(tuned, saved.Count, options.TargetPrecision).Promise("team", layers.Thresholds);
File.WriteAllText("ticket.tuning.json", tuning.ToJson());

var kept = Tuning.FromJson(File.ReadAllText("ticket.tuning.json"));
using var current = kept.Stale(schema, saved.Count) is null ? kept.Apply(schema) : null;
```

`Promise` records the precision each chosen threshold promises for the next answers; `Usage.Count` over what was
answered and settled since reports how each attribute's answers fared against it.

## Build and test

Requires the .NET 10 SDK and a stable Rust toolchain: the `Gil` project copies the decision core's native library
from `target/debug`, so build it first.

```
cargo build -p gil-ffi
dotnet build Gil.slnx
dotnet test --solution Gil.slnx
```

`Gil` is packed only when given a folder with each runtime's library — `<folder>/<rid>/libgil_native.so`,
`libgil_native.dylib` or `gil_native.dll` — and packing fails when one is missing; the native workflow
(`.github/workflows/native.yml`) builds all six and packs with them:

```
dotnet pack src/Gil/Gil.csproj -p:GilNativeRoot=<folder>
```

The form path's golden lives in `tests/fixtures/core`: `records.jsonl` holds made-up settled records, one per line
(values, sets, how each decided value was settled), and `golden.jsonl` what the decision core decides from them — the
key attributes, key concentrations, layer and coarse thresholds chosen on the first 200 records, then every suggestion for the rest, numbers
in their shortest round-trip form. Another implementation that follows docs/text-normalization.md should write the same
lines byte for byte. `text.jsonl` does the same for the normalization alone: texts that exercise each of its steps,
with the normalized text, the key and the n-grams each makes, and `keys.jsonl` for the key memory alone:
what it ranks for each single-valued decided attribute of every later record, with the same settings.

The decision core is written in Rust under `crates/gil`, whose tests write these goldens (set `GIL_WRITE_CORE_GOLDEN`
to a directory to write the files afresh) and check them, and is published as
`iyulab-gil` on crates.io and PyPI (imported as `gil`). With a stable Rust toolchain:

```
cargo test
```

`cargo test` also checks `crates/gil-review` (`iyulab-gil-review`, imported as `gil_review`; not yet published): the
statistics for checking decisions by hand — a seeded stratified sample, Neyman allocation, a stratified weighted
estimate with normal and bootstrap intervals, agreement between two reviewers. It depends on nothing, the decision core
included, and its golden is `tests/fixtures/review/golden.jsonl`, for a binding to reproduce.

Its .NET binding, `bindings/csharp/Gil.Review` (not yet published), calls the crate through a flat C ABI
(`crates/gil-review-ffi`) and reproduces that golden bit for bit. Packing it takes the native library of each runtime
(`-p:GilReviewNativeRoot=<folder>`, one `<rid>/` subfolder per runtime), which the package carries under
`runtimes/<rid>/native/`. The workflow `.github/workflows/native.yml` builds both native libraries (this one and the
decision core's, below) for linux-x64/arm64, osx-x64/arm64 and win-x64/arm64, packs the binding with them, and runs a
consumer of the package on four kinds of runner; it publishes nothing. For the tests, build the native library first;
the test project copies it from `target/debug`:

```
cargo build -p gil-review-ffi
dotnet test --solution bindings/csharp/Gil.Review.slnx
```

The decision core has a C ABI of its own, `crates/gil-ffi` (native library `gil_native`; not published): handles for a
schema, settled records read once, a decider, an open record and an ask planner, with definitions and batch results as
JSON in the core's serde form and the answer asked for while a person types written into flat structures and caller
buffers. Its .NET binding, the `Gil` project (`src/Gil`), builds schemas and settled records as typed values
(`Schema`, `AttributeDefinition`, `Record`, with time as `DateTimeOffset`) or reads them as JSON, hands answers back typed
(`Answer`, with each candidate's evidence and, when asked, the lookup's `DecisionRecord`), reproduces the decision
golden and the ask planner's golden through it, and its threshold selections, tuning round trip and usage counts give
what the C ABI gives (`tests/fixtures/core/native/selections.json`). Calls that only read a handle may run on several
threads at once; a call that changes one needs it to itself, and is refused before it changes anything when the core's
contract rules it out — a failure after that marks what it was changing broken. Each library says which version of its
interface it speaks (`gil_abi_version`, `gil_review_abi_version`), and both .NET bindings check it before their first
call. The plumbing both C ABIs share — statuses, the last error, the panic guard — is `crates/gil-ffi-support`.

Its tests are `tests/Gil.Core.Tests`, run with the rest of `Gil.slnx` as above.

## License

Apache-2.0
