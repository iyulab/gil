# Gil

Suggests the judged fields of a form — a ticket's team, a report's fault code — from the documents already settled, and
says how often a suggestion it trusts is right. It needs no model: it learns from the values settled together and from
the settled documents most like the one being filled, and how far to trust each is chosen by replaying the saved
documents in the order they were settled. The library also carries an experimental path that answers requests in front
of an LLM from confirmed answers and one-token judgments; see [Stability](#stability).

**Status: early.** The library is being built from a specification whose behaviour was measured first in a
research harness. Published on NuGet as `Gil` (with `Gil.Abstractions`), `Gil.Tree`, `Gil.IronHive` and `Gil.Sqlite`; the API may still change within 0.x —
[CHANGELOG.md](https://github.com/iyulab/gil/blob/main/CHANGELOG.md) lists each change and what to do about it.

## When to use it

Gil is for a form that is filled in again and again — support tickets, service requests, maintenance reports — where
some fields are typed and others take one of a set of values that the typed ones mostly decide, and where what a person
finally settled is kept. It suggests those judged fields one at a time:

- **From values settled together**: a value settled alongside the same value of another field, weighted toward the
  more recent.
- **From similar settled documents**: the nearest settled documents vote, compared by their characters with no model,
  or by meaning through an embedding model you choose.
- **At a precision you choose**: each layer's threshold is chosen by replaying the saved documents, so a suggestion it
  trusts carries the precision the replay saw. What falls short is still offered as an untrusted guess.

What was measured, on a public stream of aircraft maintenance reports: letting the replay choose the fields that key a
fault code with several hundred values made the key layer answer it 1.9 times as often as keying it by every field, at
a precision of 0.94 against 0.90; and the code's chapter — its first two characters — was trusted for 42% of the reports
whose code went unanswered at a target of 0.95, and was right for 97% of them.
The precision a replay promises can still be optimistic, most of all where settled documents arrive in batches —
[Filling forms](#filling-forms) says when and what to do about it. Measure it on your own saved documents before
relying on it.

It is not the right tool for a field whose value the other fields do not decide (free text, a new name), for a form
nobody settles (memory then stays empty), or for choosing between models (a router does that).

## Layout

| Project | Contents |
|---|---|
| `Gil.Abstractions` | Records and ports: the decision tree, model calls, traversal steps, telemetry sink, habit statistics |
| `Gil` | The decision core: form definitions and settlements, the form resolver and its sessions (values settled together, similar settled documents through the lexical memory, an optional model), threshold selection by replay, the document memories, call recording and pricing. No model needed |
| `Gil.Tree` | The experimental tree judgment path (`GIL0001`, `GIL0002`): the tree YAML reader/writer, the single-token judge, the greedy traverser, output contracts, the fallback generator, slot filling, the resolver (memory → tree → narrowed fallback → full fallback), habits and their promotion, deactivation and differentiation, shadows, and `ResolverFieldModel` |
| `Gil.IronHive` | Chat and embedding models through any IronHive generator, with ready-made ones for OpenAI-compatible servers (retry rules for busy shared servers, the response kept as received). Optional: implement `IChatModel` and `IEmbeddingModel` yourself and `Gil` needs nothing else |
| `Gil.Sqlite` | The telemetry store: every trace and model call, habit statistics, promotion and review logs in one SQLite file whose layout is the contract below. Optional: `Gil` takes any `ITelemetrySink`, and a consumer that keeps no telemetry ships no SQLite native library |
| `Gil.Tests` | Unit tests and the compatibility fixture writer |

The telemetry file layout is a contract: analysis tools read it directly, so columns may be added but never
renamed or repurposed. `Writes_the_compatibility_fixture` produces a small synthetic store other
implementations can open to check they read the same layout (set `GIL_COMPAT_FIXTURE` to choose where).

The same contract is checked the other way on every build: judgments, traversal, prompts, the resolver (with and
without memory, including memory failures), shadows, promotion, deactivation, differentiation and tree files are
replayed from synthetic fixtures under `tests/fixtures/conformance`, which the reference implementation produced from a
made-up support task and a deterministic model. Set the matching `GIL_COMPAT_*` variable to replay a recorded run instead.

## Stability

The library has a supported surface and an experimental one.

**Supported** is filling forms: `FormResolver`, `FormSession`, `FieldMemory`, `ThresholdSelection`, the form
definitions and settlements, the document memory `LexicalMemory`, and the chat and embedding models. Within 0.x a release may still change it; each such change is listed in the changelog under
**Breaking** with what to do.

**Experimental** types carry `[Experimental]`. Using one is a compile error until its id is suppressed, and a marked
type may change shape or be removed in any release without a migration note.

| Id | Covers |
|---|---|
| `GIL0001` | The tree judgment path: `Resolver`, task and tree definitions, `TreeYaml`, traversal, one-token judging, output contracts and fallback generation, `ResolverFieldModel` |
| `GIL0002` | Habit promotion: proposals and their review, deactivation, differentiation, shadow evidence |
| `GIL0003` | Recording and pricing: `ITelemetrySink` and `SqliteTelemetryStore`, the call recorders, `EnergyModel`, statistics, rebuilding a memory from the feedback log |
| `GIL0004` | A document memory other than `LexicalMemory` on the form path: `EmbeddingMemory`, `CircuitBreakingMemory`, a memory of your own, and the `FormResolver` and `ThresholdSelection` overloads that take any `IMemory` |
| `GIL0005` | Asking advice on the form path: `AskPlanner` and `AskAdvice` — which observed field to ask a person for first, within a budget of typing |

Suppress the ids of what you use, in the project file or around the code:

```xml
<PropertyGroup>
  <NoWarn>$(NoWarn);GIL0001;GIL0003</NoWarn>
</PropertyGroup>
```

[Filling forms](#filling-forms) needs none of them — a telemetry sink passed to `FormResolver` (`GIL0003`) and a
document memory other than `LexicalMemory` (`GIL0004`) are the optional pieces there that do. The walkthrough under
[Usage](#usage) is the tree judgment path: it needs `GIL0001`, `GIL0003` and `GIL0004`, and `GIL0002` for its habit
promotion part.

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
dotnet add package Gil
dotnet add package Gil.Tree       # the tree judgment path this walkthrough uses
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
abstains instead of reaching a model. Its similarities are on their own scale: pick the task's `MemoryThreshold` by
replaying your feedback history rather than reusing an embedding memory's value.

```csharp
var lexical = new LexicalMemory();
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
`GilDiagnostics.Name` (`"Gil"`): a `gil.resolve` span per request with the task, mode, memory outcome (`off`, `hit`,
`miss`, or `failed` — a lookup that failed and was treated as a miss), energy and confidence, a `gil.call` span under
it per model call with its role, node and tokens, and the metrics `gil.resolutions` (by task, mode and memory
outcome), `gil.resolution.energy`, `gil.resolution.duration` and `gil.call.energy`. With OpenTelemetry, add
`AddSource(GilDiagnostics.Name)` and `AddMeter(GilDiagnostics.Name)`. Provider spans and token metrics (`gen_ai.*`) come
from the model client, not from Gil, and nest under `gil.call`.

## Filling forms

A form is a different shape of task: a document with several fields, some typed by a person (observed) and some to
be suggested (judged), settled one at a time. `FormResolver` suggests the judged fields without a model. It learns
from settled documents only: which values were settled alongside which values of the other fields, and optionally
which settled document is most similar. Everything in this section is the supported surface (see
[Stability](#stability)).

```csharp
using Gil;
using Gil.Forms;
using Gil.Memory;

var form = new FormDefinition("ticket",
[
    new FieldDefinition("reporter", FieldRole.Observed) { UseAsEvidence = false },
    new FieldDefinition("component", FieldRole.Observed),
    new FieldDefinition("summary", FieldRole.Observed),
    new FieldDefinition("team", FieldRole.Judged) { MemoryThreshold = 0.5, KeyThreshold = 0.6 },
    new FieldDefinition("severity", FieldRole.Judged) { Candidates = ["low", "medium", "high"], Policy = FieldPolicy.ConfirmRequired },
], PromptLanguage.English);

var forms = new FormResolver(new FieldMemory(), new LexicalMemory());
await forms.RebuildAsync(form, saved);
```

The second argument is the document memory, `LexicalMemory`: it compares documents by their characters and needs no
model, and it is the only kind the supported surface takes. Pass `null` for a form without one. A memory that compares
documents by meaning (`EmbeddingMemory` over any `IEmbeddingModel`), one behind a `CircuitBreakingMemory`, or one of
your own goes through the overloads that take any `IMemory`, which are experimental (`GIL0004`): on two public
datasets we replayed, the lexical vote of ten neighbours answered more, and more precisely, than an embedding
memory's nearest document, so the slot stays narrow.

`UseAsEvidence = false` keeps a field's value out of every other field's suggestions. Use it for fields that identify a
person, so that "this person, therefore this outcome" never hardens into memory. `ConfirmRequired` tells the
application that a field's suggestion should never be filled in without a look. `Off` never suggests the field at all.

Open a document once and pass values as they arrive. Every event returns fresh suggestions for the open fields whose
evidence changed. Settling the fields in order therefore keeps the rest up to date, and accepting a suggestion then
correcting it later leaves only the correction in memory:

```csharp
var session = forms.Open(form, "tickets/0412");
await session.ObserveAsync("reporter", "Kim");
var suggestions = await session.ObserveAsync("summary", "VPN drops every ten minutes");
var updated = await session.SettleAsync("team", Settlement.Accept("network"));
var document = session.Snapshot();
```

Say how a value was settled. `Settlement.Accept` and `Settlement.Correct` are for a person who had the suggestion in
view. `Settlement.Enter` is for a person who settled the field with no suggestion shown — because the application
withheld it, kept it below the threshold it shows, or had none. `Settlement.Fill` is for the application settling the
suggested value on its own, with no person confirming it. The snapshot keeps the kind of each judged value in
`SettledDocument.Settlements`; save it with the document. A value put back with `Settlement.Restore` is not known to have
been settled any way. A filled value stays in the document, but memory does not learn it and choosing thresholds does not
ask about it. Remembered, a suggestion's own answer would come back as evidence for itself. Replaying a public stream
with nine in ten trusted answers filled, a replay that learned from them promised 0.97 where 0.73 was right, and one
stream's key layer ended up answering every document with 0.11 right. Learning only from what a person settled kept the
promise within a few points of what was right.

A field left empty is not settled: leave it out of the document rather than settling an empty string. An empty string
is a value like any other, so memory would learn it and suggest it to the next similar document; a field the document
does not have is neither learned nor asked about when thresholds are chosen, and an empty value of another field is no
key. Where "unknown" is a real answer, make it a value of the field's list and settle it.

An application that fills values on its own still needs some of them checked, or nothing tells it when the fills go
wrong. Draw the decisions to check at random, with a probability fixed before the draw — the same for every decision
unless some keys need watching more closely — and have someone settle the field again without seeing the filled value:
`Settlement.Check(value, probability)`. Memory learns the checked value in place of the filled one, and choosing
thresholds counts each check as one ÷ its probability decisions, the filled decisions it was drawn from, so checking some
keys more often does not tilt the promise toward them. Name `SettlementKind.Check` in `counting` beside the kinds a
person settles. A decision picked because it looked doubtful has no such probability and is not a check: counted as one,
it would lean the promise toward the doubtful ones. Few checks weigh much but say little, so
the fewest answers a threshold must rest on is held to their effective number (`ThresholdChoice.EffectiveAnswered`).

Replaying two public streams with nine in ten trusted answers filled and the tenth checked, counting the checks by their
weight kept the promise within two points of what was right in both, where counting them once had promised more than two points
less than was right in one. Weighting makes the promise honest rather than generous: where the answers had been doing
worse than promised, thresholds rose and about 8% fewer answers were right; where better, slightly more were.

```csharp
var filled = forms.Open(form, "tickets/0413");
await filled.ObserveAsync("summary", "VPN drops again");
var answer = await filled.SuggestAsync("team");
if (answer.Answered)
{
    await filled.SettleAsync("team", Settlement.Fill(answer.Candidates[0].Value));
    if (Random.Shared.NextDouble() < 0.05) // the probability is fixed before the draw
    {
        await filled.SettleAsync("team", Settlement.Check(checkedByPerson, 0.05));
    }
}

var checkedLayers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30, counting: [SettlementKind.Enter, SettlementKind.Check]);
```

The check is a second settlement of the same field in the same session, after the fill; the snapshot then keeps the
checked value, its kind and its probability. In a public bug tracker, values a machine filled were changed by a person
within 30 days 26% of the time, values a person chose 7%. Within the same component the gap grows rather than shrinks —
the machine's values were changed 4.8 times as often as people's (3.5 to 6.7), so they are wrong more often, not merely
placed where values move. Filled values need the checking.

Some values must not be missed: answering a dangerous grade as a harmless one costs far more than asking. A threshold
cannot hold such a value back where it is rare — a key under which it was settled once among hundreds of other values
still scores its usual value high. Name those values in `FieldDefinition.MustCatch`, and once one of them has been
settled under a key the document has, no layer answers the field for that document — neither the key layer nor a
similar document, whose neighbours rarely include the value that is rare under the key — and its coarse level is not
offered. Its values under such a key are still offered, as guesses. Keyed by a field drawn from many unrelated values
(a host, a user), one such value settled under each soon holds back every document, so key the field by what decides
it (`FieldDefinition.KeyFields`). Replaying public
system logs whose risk grade a model had judged, naming grades 3 and above cut the lines graded 3 or more that were
answered below 3 from 2 to 1 of 1,425, while keeping 81% of the lines the key layer answered. It reaches only keys where
such a value was already settled: the first one under a key is caught only if someone checks the answer, so it does not
replace checking.

A replay's precision runs over every document it saw, so a layer that did better long ago keeps it high after the layer
has changed, and the threshold was chosen on those very answers, so their precision flatters it — most of all when they
are few. `ThresholdChoice.Promise` answers both. It takes the precision of the answers the threshold gives among the
latest `ThresholdSelection.RecentDocuments` documents (the whole replay's when those are too few to rest on) and lowers
it to the bottom of its Wilson score interval at one standard error for the answers it rests on, which
`PromiseAnswered` reports. On many answers the two hardly differ; on a few dozen the promise sits several points
below. Show it as the promise when the thresholds were chosen layer by layer — `SelectLayersAsync`, or
`SelectKeyThreshold` for a field with the key layer alone. When `PromiseAnswered` is small, say so beside it: the
number is a floor, not a forecast.

Replaying three small public forms of about 670 records each, with thresholds chosen again whenever the records grew by
a tenth, the similar document layer's promise without that lowering ran 9.1 points above what its answers then
delivered, and 1.5 points above with it; the key layer's moved from between 0.1 below and 6.9 above to between 3.5 below and 1.6
above. On the first 5,000 records of two larger public streams, where the unlowered promise had already been right,
the lowered one came out 2.4 and 4.5 points below. Over long replays the recent window is what matters: on two public
streams of 80,000 records, the recent-window promise stayed within half a point of what its answers delivered where the
key layer's whole-replay precision ran 2.5 points high, and on a stretch where that precision had drifted 9.6 points
above the answers it was 2.7 points above. The window is 5,000 documents, so on a stream shorter than that the
promise is read on the whole replay and trails a change: in a synthetic log stream of 3,000 records where one
template's grade changed at the 2,000th, the key layer promised 2.2 points more than the next 500 answers kept.
Reading it on the latter half of the replay instead turned that into 6.8 points too few on the 500 after, so the
window stays as it is; what moved first was the state under the changed key (below), whose precision there fell to
0.69 while the field's promise still read 0.91. Neither mends a threshold chosen on the wrong lookups: a similar document
threshold chosen apart from the key layer (`SelectAsync` on a field that also has a `KeyThreshold`) promised 2.4–2.6
points more than it kept. For the key layer, `ThresholdReplay.Keys` says, under each key that backed a best
value, how many lookups it had, how many the chosen threshold answered and how precise those were (weighted, as
checks are), and whether it answers under that key now — what an application checking its answers reads to decide how
often to check each key. A key reads as keys compare values — `template: t07` for a value settled as `T07` (see
docs/text-normalization.md). That policy stays the application's: a key answered rightly many times may be checked less
often, as long as the probability of each draw is still fixed before it.

Each field is tried in a fixed order, and `FieldSuggestion.Source` says which layer the first candidate came from:
1. Values settled alongside the values the document already has, when the keys backing them score high enough
   (`KeyThreshold`).
2. The value the most similar settled documents vote for, when the field sets `MemoryThreshold`, a document memory is
   given, the vote is decided enough, and the documents voting for it are about as like the draft as those the
   threshold was measured on (`MemorySimilarityFloor`).
3. A model, when one is given (`IFieldModel`; `ResolverFieldModel` puts the resolver above behind it, one task per
   field). It is asked only when neither memory had anything, so it never overrides a value the document supports.

A layer answers only at or above its threshold. What falls short is still offered, after the layers that answered, as
a guess: values under a key below `KeyThreshold`, then the value settled most often for the field, then the value
similar documents vote for below `MemoryThreshold`, then the field's other values by how often they were settled.
Replaying public streams of settled documents ranked them so: the nearest document below its threshold was right less
often than a weaker key or the most frequent value, but far more often than the next most frequent ones. A field without
`MemoryThreshold` makes no promise from similar documents, but with a document memory they still vote and the value is
offered in the same place. `FieldCandidate.Trusted` marks which is which, and `FieldSuggestion.Answered` says whether
the first candidate is an answer at all. When it is not, leave the field to the person: do not fill a guess in, and do
not mark it as the suggestion. Listing guesses as unmarked choices the person may pick is another matter. Replaying a
public stream of settled documents, showing the first guess that way saved about a third of the typing, even after
charging for the time to read it, against about one percent for answers alone. The first guess was also wrong more often
than right, though, and a value already filled in or highlighted is easily accepted without a look. A value's score
adds up its strength under each of the document's keys — how pure the key is for it, discounted when the key was seen
only a few times — so several keys that agree outweigh a single one, and the same score both ranks the values and is
compared with `KeyThreshold`. Replaying a public stream of settled documents with thresholds chosen for a precision of
0.8, this let the key layer answer about three times as often as trusting the strongest key alone, at the same
precision. A choice among a handful of values that every document has rarely decides another field, and without
`KeyThreshold` its values are guesses, so it never outranks a similar document that meets its threshold.

The similar document layer lets the nearest `SimilarDocumentVotes` documents vote — ten by default. Each votes for its
settled value with its similarity; the value with the most weight is the candidate, and its score is the vote's margin:
the winner's weight less the runner-up's, over all the weight cast, from 0 for a tie to 1 when every voter agrees. That
score is what `MemoryThreshold` is compared with, so a near document that its neighbours contradict is no longer trusted
on its similarity alone. Only documents whose value lies in the field's domain vote. Replaying public streams of settled
documents with thresholds chosen for a precision of 0.8, ten voters answered 1.8 times as many documents right as the
nearest document alone in one stream and 1.2 times in another, at a higher precision (0.89 and 0.91 against 0.83 and
0.86). `SimilarDocumentVotes = 1` lets the nearest document decide alone on its similarity, as before 0.16.0; so does a
memory that ranks only its nearest document. Choose `MemoryThreshold` again after changing it.

The margin says how far the neighbours agree, not how closely they resemble the draft. A draft written in words no
settled document uses still has nearest documents — ones that share only an opening phrase or another field — and they
can agree. A threshold chosen by replaying documents written the usual way never saw such a draft, and when that replay
was right every time it chooses a threshold low enough to let them through. So the replay also chooses a floor: of the
answers at or above the threshold, the similarity of the nearest document voting for the answer that only a hundredth
of them fell below. Set it as `MemorySimilarityFloor` with the threshold; below it the vote is offered as a guess. In a
synthetic stream where every settled document used the same words for each topic, ten voters with the threshold alone
answered about half of drafts reworded in other words wrongly, and none with the floor. Replaying public streams with
drafts stripped of their free text or of its topic words, the floor cost under one percent of the answers to drafts written the
usual way and answered no more of the stripped ones wrongly. With a single voter the threshold already is a
similarity, and no floor is chosen.

To show what a similar document's candidate rests on, create the resolver with `similarDocumentCount`: each suggestion
then carries `SimilarDocuments`, the most similar settled documents with their similarity and settled value; the
candidate's own document is the nearest of them with its value. They come from the same lookup, so a person sees the evidence the suggestion was made
from — and whether the neighbours agree — at no extra cost. A document being edited is never among them: the saved
version of it is passed over for the next most similar document.

On a machine without a large model, leave the model out: measured on requests a lexical memory did not answer on its
own, small local models (2B and 4B, quantised) were right less often than that memory's nearest document, and took
seconds per field on an office laptop.

When no layer has a candidate, a person decides. Only a model reports `Confidence`: a frequency or a similarity is not
a probability. Give `ResolverFieldModel` the same sink and name each task `form/field`, and a model's suggestion is
traced like any resolved request — `FeedbackAsync` on its trace id then reaches the tree.

Where saving a document is what settles it, a draft must not reach memory. Ask with the values on screen instead of
opening a session, and put the document once it is saved; the saved version of the same document is left out of its
own evidence:

```csharp
var onScreen = await forms.SuggestAsync(form, "tickets/0412", valuesOnScreen);
var teamOnly = await forms.SuggestAsync(form, "tickets/0412", "team", valuesOnScreen); // one field, when only it is shown
// … on save:
await forms.RebuildAsync(form, [savedDocument]);
```

Save the snapshot the way you save documents. At startup, or when documents change elsewhere, pass them to
`RebuildAsync`. A document's contribution is always what its current values imply, so live settling and rebuilding
can be combined without counting anything twice.

Every `SettledDocument` carries `SettledAt`, when it was last settled — a file's last write time serves. Where
documents disagree, the later settlement wins: `FieldMemory` weighs each settlement by how many came after it under
the same key (`recencyDecay`, 0.95 by default — a correction overtakes an older practice without first outnumbering
it), ties go to the latest settlement, and documents whose evidence reads the same are one case that a
similar-document lookup answers with its latest settlement. The result is the same whatever order documents arrive in. Reopen a saved document from its snapshot.
It comes back as it was saved: its values, the order they arrived in, its time and how each value was settled. Only
settling a field moves the time on:

```csharp
var reopened = forms.Open(form, saved[0]);
```

Putting values back one by one with `Settlement.Restore` also keeps the time. But it takes the order they are put back in
as the order they arrived in, and does not know how they were settled.

The right thresholds move as memory grows. Choose them again from time to time — for instance when memory has grown
by a tenth — by replaying the saved documents. Both are chosen in one replay, in the order the resolver consults the
layers: a similar document answers only where no key did, so its threshold is chosen on those lookups alone. A layer
without a `Chosen` threshold should not answer the field on its own; its `MostPrecise` still says how close it came —
the best precision it reached on at least `minimumAnswered` answers — so the application can say by how much the field
falls short:

```csharp
var layers = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30);
var team = form.Field("team") with
{
    KeyThreshold = layers.Key.Chosen?.Threshold,
    MemoryThreshold = layers.Memory.Chosen?.Threshold,
    MemorySimilarityFloor = layers.Memory.Chosen?.SimilarityFloor,
};
var tuned = form.WithField(team);
```

Put the field back with `WithField`; the form is a record whose fields are checked together, so it is not changed in
place.

Which target to ask for is yours to set, and it follows from what a wrong answer costs. Count a value a person enters
with no suggestion as 1, a right answer as nothing, and a wrong one — noticed, undone and entered again — as ρ. An
answer right with probability p then costs ρ(1 − p) against the 1 of asking, so answering pays only above a precision of
1 − 1/ρ: 0.5 where a wrong value costs two entries, 0.8 at five, 0.9 at ten, 0.95 at twenty. Below that line a threshold
that keeps its promise still loses — each answer it adds costs more than it saves. Replaying a public stream of
biological sample records with costs counted this way, thresholds chosen for 0.8 answered right 0.88 of the time — a
loss once a wrong value costs ten entries; chosen for a target at or above 1 − 1/ρ, they cost less than asking at every
ρ from 2 to 20. When you do not know ρ, time how long undoing a wrong value takes against entering one.

A value a person accepted with the suggestion in view may be right because it was suggested. A value the application
filled was never checked. To know how right a layer is on its own, count only the settlements that saw no suggestion.
Every document is still replayed into memory; only the lookups counted change. A document that does not say how a field
was settled is counted only when `counting` is left out:

```csharp
var unprompted = await ThresholdSelection.SelectLayersAsync(new FieldMemory(), new LexicalMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30, counting: [SettlementKind.Enter]);
```

Where a field's values name their class in their leading characters — the chapter of a fault code, the class of a
patent classification, the group of a product code — a value the layers cannot answer can often still be given its
class. Set `FieldDefinition.Coarse` to a `CoarseLevel` with the prefix length, and when no layer answers the value
`FieldSuggestion.Coarse` offers a prefix: the one the values under the document's keys back by their summed scores,
else the one the similar documents vote for. It is trusted at the level's own thresholds, chosen after the field's with
`ThresholdSelection.SelectCoarseAsync` on the lookups the value layers leave, and is a guess otherwise. It never enters
`Candidates`, which hold settled values only. The class is worth what follows from it — routing, a reviewer, a section
of the form — more than typing: a right two-character chapter saves one keystroke of four. Replaying public streams with
thresholds chosen on the first ten thousand documents, the chapter of an
aircraft fault code was trusted for 42% of the reports whose code went unanswered at a target of 0.95, and was right
for 97% of them; the class of a patent classification, for 10% of the patents unanswered at 0.8, right for 85%.

Where a value's class is not in its characters but in a table — the panel that reviews a device code, the category of a
product — give the table instead: `new CoarseLevel(classes)`, value to class. The values stay as they were settled and
the classes are read off them, so the table can change without settling anything again. A value the table does not list
is a class of its own, so list every value — those without a known class under a class named for them. Replaying public
medical device recall records with the review panel of each product code from the public classification table — 19
panels — the panel was trusted for 54% of the records whose code went unanswered at 0.8, and was right for 90% of
them; the code's own first letter, as a prefix, managed 9%.

**Asking for a field first** (experimental, `GIL0005`). When no layer answers a judged field, one more observed value
from the person — the component a ticket is about, the part a fault was found on — may let one answer, or a class be
trusted. Asking every time costs every one of those values typed. `AskPlanner` learns, from settled documents in the
order they were settled, what asking each field gained per character, and advises an open document within a budget:

```csharp
var planner = new AskPlanner(form, "team", askable: ["component"], budget: 0.5);
var replay = new FormResolver(new FieldMemory(), new LexicalMemory()); // starts empty: no later document leaks in
foreach (var earlier in saved)                                         // in settlement order
{
    await planner.LearnAsync(replay, earlier);
    await replay.RebuildAsync(form, [earlier]);
}

var teamNow = await forms.SuggestAsync(form, "tickets/0412", "team", valuesOnScreen);
foreach (var advice in planner.Advise(teamNow, valuesOnScreen)) // empty fields, highest gain per character first
{
    if (advice.Ask) { /* ask for advice.Field before the rest */ }
}
```

What a suggestion is worth: 1 for a value answered right, and for a trusted class that holds the settled value, the
share of the field's values it rules out. A field's gain is kept under the class the suggestion without it showed and
whether that class was trusted, starting from 0.1 gained over 10 characters, and asking is advised where the gain is
among the highest that, counted down, fill the budget — a share of the typing that asking every time would take — while
asking has spent no more than that share; the planner counts as though its advice was followed. Learn from a resolver whose memory holds only the
documents before each one, then keep calling `LearnAsync` with the live resolver as documents are settled. A field the suggestion already answers is not worth asking, and with several
fields open the advice for each was learned with only that one missing. Replaying a public stream of medical device
recalls (asking for the recalling firm to answer the product code, ten thousand records after thirty thousand) with a
budget of 0.49, advice spent 49% of the typing and gained 0.032 per record — 60% of what asking every time gained;
on a public stream of aircraft fault reports (asking for the part to answer the fault code), 48% and 0.094 — 68%.

A chosen threshold also holds only for the version of Gil that chose it. A threshold rests on how a layer scores its
candidates and on what the replay asks, and a release may change either: 0.9.0 changed the key layer's scale, 0.10.0
and 0.11.0 what the replay asks, 0.16.0 the similar document layer's scale, and 0.17.0 added its similarity floor. When you store a threshold, store the Gil version with it and choose again when that
version changes:

```csharp
var gilVersion = typeof(FormResolver).Assembly
    .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion;
```

What an upgrade moves can be counted before you take it. Replay your saved documents once with each version — two small
programs that differ only in the version of the `Gil` package — choosing thresholds on the earlier documents and asking
for each later one before putting it into memory, and write down each suggestion's first candidate, whether it was
answered and its coarse class. Compare the two lists document by document. A release marked **Breaking** changes a
contract, not necessarily your answers: replaying two public streams (medical device recalls and aircraft fault
reports, ten thousand suggestions each after ten thousand to choose from), 0.21.0, 0.22.0, 0.23.0 and 0.24.0 answered
every one of the twenty thousand the same way. Choosing again after an upgrade is still the rule; the count says what
it will move.

The replay looks each field up with the values that had arrived before it was settled — what its last suggestion was
made from. `Snapshot()` records that order in `SettledDocument.Arrival`; save it with the document. Without it, observed
values count as there from the start and judged fields as settled in the form's order. Even so, the precision a replay
reports can be optimistic. `targetPrecision` is checked on the documents the threshold is chosen on. Where settled documents
come in batches — several at once from one source, alike in their observed values — whether a near-identical
document's value is right tends to hold or fail for the whole batch. The replay then sees fewer independent cases than
it counts. With this version, key layer thresholds chosen once for a target of 0.8 on the first forty thousand documents of
two public streams — an issue tracker and aircraft maintenance reports — answered right 0.85 and 0.90 of the time on
the forty thousand that followed, and 0.82 to 0.91 for the six fields of a smaller stream of biological sample records.
Version 0.6.0, on two other streams of sample records, saw 0.63 to 0.74 at 0.8 and
0.64 to 0.72 at 0.7; wider statistical margins did not close that gap. To know the precision a field actually delivers,
check it on your own documents: choose on those settled up to a date and count the answers on those settled after it.

Settle a record that arrives again — a retransmission, a re-save — under the same document id. The memories keep one
version per id, and the replay asks a document settled again without its earlier version, as a suggestion for a saved
document does. Under a new id the copy is new evidence: copies answer each other in the replay, and the threshold
promises far more than other documents get. Replaying a public stream in which about half the documents arrived twice,
copies under new ids made the key layer answer six times as often with 0.67 right against a promise of 0.91; under
the same ids, it answered as if no copy had arrived.

The two layers answer different kinds of documents. The key layer answers where a value the document already has
decides the field. The similar document layer answers the rest, which are harder. A form whose judged fields rest only
on observed fields (`DependsOn` naming observed fields alone) gets most of its answers from similar documents. In a
replay of a public stream of settled documents with the nearest document deciding alone, that layer's answers were
right about seven times in ten at most, even
among the most similar documents: documents whose observed values read almost the same had different settled values.
Thresholds chosen there for a target of 0.8 delivered 0.66 to 0.72 on the documents that followed. Where settled
documents come in batches that share their observed values, expect the similar document layer to fall short of a high
target, and read even `MostPrecise` as optimistic: it is measured on the same documents it was chosen on.

Which fields key a judged field matters most when the field has many values. A field that rarely decides it still
adds to the score of the values settled alongside its own: a value drawn from many, each seen only a few times — a
model number, say — lends a few wrong values most of their strength and pushes them above the right one. Name the
fields that decide the field in `KeyFields`. To find them in the saved documents, replay the key layer with
`ThresholdSelection.SelectKeyFields`. It weighs a set of fields by what its key layer gains at the target precision —
the right answers less the target times the answers, the amount the threshold itself is chosen to make largest — so an
answer counts only by how far it is right beyond the target. It tries each field the field rests on alone, adds them in
order of that gain while each addition gains more, and names a set only if it gains more than every field together;
otherwise `KeyFields` comes back null. Counting answers alone would drop a field that names the very thing the field
describes (a customer, a site, a piece of equipment): alone it answers only where that thing comes again, so a broad
field answers more often — and wrongly on exactly those. Then choose the thresholds again with the fields it named,
since a narrower set changes the scores:

```csharp
var narrowed = ThresholdSelection.SelectKeyFields(() => new FieldMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30);
var keyed = form.Field("team") with { KeyFields = narrowed.KeyFields };
```

Replaying a public stream of maintenance reports, it chose two of thirteen observed fields for a fault code with several
hundred values: the key layer answered 1.9 times as often as with every field, at a precision of 0.94 against 0.90.
These were the two fields a learned weighting of the fields ranked highest, and resting on them answered 2.2 times as
often on the documents that followed, at 0.95 against 0.89. It costs one replay per field tried and one per addition.

`KeyFields` narrows the key layer only. The similar document layer and a model still read every field the field rests
on (`DependsOn`): matching a whole document discounts a line that does not decide the field rather than counting it,
and cut down to a field or two the lookups read alike. Replaying a public issue tracker with both layers on, resting a
component field on the one field the key layer chose (`DependsOn`) answered half as often; keying it by that field
(`KeyFields`) answered rightly 1.02 times as often as every field, on the same documents at the same target. The key
layer took more of the answers and the similar document layer fewer, so precision moved from 0.89 to 0.88 — above the
target of 0.8, which is what the thresholds promise. On the maintenance reports above, with the similar document layer
and a coarse level on and a target of 0.95, keying the fault code by the one field chosen answered it rightly 1.36
times as often as every field, at 0.96 either way; resting it on that field had answered less often than every field.

How far a key seen only a few times is trusted depends on its field. A value's strength under a key is its weighted
count there over the key's weighted total plus a concentration, 1 unless set: a key seen once lends its value half its
strength, whatever field it comes from. A field that names the very thing the judged field describes — a reference to
another form's document, an equipment or customer number — keeps the value its keys were first seen with, so a key of it
seen once is as good as settled; a broad field whose keys mix values deserves less than half. Choose a concentration for
each field from the saved documents with `ThresholdSelection.SelectKeyConcentrations`. It replays them in the order
they were settled and asks, before each, how often a key seen before brought a value new to it; the concentration under
which that was likeliest is the field's. Two predictions from a key seen once are counted for every field besides, so a
field whose keys were seldom seen again stays near 1. Then choose the thresholds again, since the scores move:

```csharp
var concentrations = ThresholdSelection.SelectKeyConcentrations(new FieldMemory(), form, "team", saved);
var trusting = form.Field("team") with { KeyConcentrations = concentrations };
```

Replaying a year of public food inspections of one area (701 inspections, 382 establishments), where a risk level is
set per establishment and an inspection's kind is drawn from a few, the establishment field came out at 0.004 and the
kind at 0.23. Of the 319 inspections of an establishment inspected before, the key layer had answered 291 — 12 wrongly,
following the kind against the establishment's own record — and abstained on 28; with the concentrations it answered
313, all right, and abstained on 6. Establishments seen for the first time were answered as before (270 at 0.92, then
266 at 0.92). On a public issue tracker and the maintenance reports above, where no field names what is judged, the
concentrations ranged from 0.2 to 5 and the answers given rightly moved by −0.2% to +1.9%, every threshold keeping its
target.

A concentration near zero is also what a field gets when it was written from the judged value itself — a copy of it, an
identifier assigned to each value — and such a field may not be there yet while the document is being filled in. With
fields like these planted beside the judged field of seven public streams of settled records, a copy and an identifier
per value came out below 0.001 in every stream, and none of the 141 fields the records really had came out below 0.14.
Before trusting a field whose concentration is under 0.01, check that it is there when a suggestion is asked for, as an
establishment is; if it is filled only once the judged field is settled, save `Arrival`, so the replay looks the field
up without it. A field computed from the judged value that folds several values into one code reads to the replay as
strong evidence and comes out like any other field: only `Arrival` keeps it out.

A key is a short value that recurs. `FieldMemory` uses a value as a key only up to `maxKeyLength` characters once
normalised (60 by default): free text rarely recurs, and a value that never repeats keys nothing. Where the evidence
is a line of text that differs from one document to the next only in its variable parts — numbers, addresses,
identifiers — give the form an observed field that holds the line's shape instead: the text with each variable token
replaced by a placeholder, or a short id of that text. Keep the text itself as another field for the similar document
layer to read, and name the shape in `KeyFields`. Whether a key recurs can be counted before replaying anything: the
share of saved documents whose key was seen in an earlier document bounds how often the key layer can answer.

When another form has already settled the same subject — the same order, case or applicant — give its settled values
to this form as observed fields. The key layer learns from the settled documents which of their values go with which
of this field's, and the similar document layer reads them as evidence; the two forms' values need not map one to one.
Replaying a public stream of patents classified under two schemes, with the class one scheme had settled given as an
observed field of the other, the field was answered rightly 1.8 times as often on the documents that followed, at the
same precision (0.86); the same values shuffled between documents answered no more often than without them. Only a
value the other form has settled is evidence: one still open there may change.

When a person starts typing into a judged field, ask again with what has been typed rather than dropping the
suggestion. Ask for that field alone, with the text typed so far — `SuggestAsync` with the field's name, or a session's
`SuggestAsync(field, typed)`: typing changes no other field's suggestion, and where each field looks up similar
documents, asking for the whole form again at every pause repeats every lookup (a form with five such fields and a
hundred thousand saved documents took about 0.2 s a pause; the field alone takes one lookup). The field stays open, and
its suggestion offers only values that begin with the text (ignoring case; Korean by keystrokes, so a syllable still
being composed — "바" on the way to "박" — begins the values it leads to). Only the key layer answers then, held to a
threshold for that many characters typed, `TypedKeyThresholds`; a value trusted with fewer characters typed, or
before typing, stays trusted while the text still leads to it, so typing on never withdraws it. A person types only
where the suggestion before did not do, and the documents left are the harder ones, so a threshold chosen on every document promises more than it keeps
there: choose them with `SelectTypedKeyThresholds`, after `KeyThreshold`, which replays each document as typed from its
start and chooses each length's threshold on the documents typed that far. Pass the form as it serves suggestions — the
field's `DependsOn` and `KeyThreshold` set — since the replay asks the key layer as a suggestion would and leaves to the
typed thresholds the documents that `KeyThreshold` did not answer rightly; choose `DependsOn` first, then
`KeyThreshold`, then these. For a field typed into, choose `DependsOn` with `typedLongest` too: the fields that decide
a field before typing can blur it once a prefix has narrowed its values, and the choice then counts answers while
typing as well and brings the typed thresholds for it (`Typed`):

```csharp
var typedTeam = form.Field("team") with { TypedKeyThresholds = ThresholdSelection.SelectTypedKeyThresholds(new FieldMemory(), form, "team", saved, targetPrecision: 0.9, minimumAnswered: 30).Thresholds };
var whileTyping = await forms.SuggestAsync(form, "tickets/0412", "team", valuesOnScreen, typed: "ne");
```

Replaying a public stream of settled documents with thresholds chosen this way on its first half, the first typed
character was where most of the gain lay: the judged fields of the second half were answered rightly about thirteen
times as often as before typing, at a precision of 0.86 against a target of 0.8 — a value kept while typing on counts
once, so record feedback once per value, not once per question — and a person was spared 9% of the characters of the
settled values, against 1% before. A person who types on without looking saw no trusted value withdrawn. A screen that shows a list rather than one value can instead keep the candidates that begin with the typed
text: with twenty candidates, filtering by the typed text saved about half of the keystrokes, against about a third
when the list was shown once and not filtered.

A judged field can take several values — tags, the topics a document covers, the answers to a multiple-choice
question. Mark it `Multiple`; its values are settled as a set, in `SettledDocument.Sets` rather than `Values`, and
order carries no meaning. Each value is remembered and suggested on its own, so several candidates can each be trusted
against `KeyThreshold`, and the values a person has already chosen are evidence for the rest. A session keeps such a
field open after it is settled and goes on suggesting the remaining values:

```csharp
var paper = new FormDefinition("paper",
[
    new FieldDefinition("venue", FieldRole.Observed),
    new FieldDefinition("topics", FieldRole.Judged) { Multiple = true, KeyThreshold = 0.8 },
    new FieldDefinition("main", FieldRole.Judged) { CandidatesFrom = "topics", KeyThreshold = 0.8 },
], PromptLanguage.English);

var session = forms.Open(paper, "papers/0107");
await session.ObserveAsync("venue", "ACL");
var rest = await session.SettleAsync("topics", Settlement.Set(["parsing"]));   // suggests the other topics
var saved = session.Snapshot();                                                 // saved.Sets["topics"]
```

Choose its threshold with `SelectKeyThreshold`, which replays each document as values are picked one after another.
A value that has a main one among several is better a second, single-valued field that takes its candidates from the
set (`CandidatesFrom`): once the document's values are chosen, the main one is offered only among them. The same holds
for any field whose values depend on another's, such as a subcategory within its category; while the other field is
empty, nothing is narrowed. The similar document layer does not suggest a field with several values, so such a field
takes no `MemoryThreshold`. Replaying a public collection of
patents, each classified into one or more of 37 technology classes, with one class of a document chosen, using the
chosen class as evidence put a remaining class first slightly more often (0.46 against 0.44) and trusted about one and
a half times as many of the remaining classes at the same precision (0.79 against a target of 0.8).

## Build and test

Requires the .NET 10 SDK.

```
dotnet build Gil.slnx
dotnet test --solution Gil.slnx
```

The form path's golden lives in `tests/fixtures/core`: `records.jsonl` holds made-up settled documents, one per line
(values, sets, how each judged value was settled), and `golden.jsonl` what this implementation decides from them — the
key fields, key concentrations, layer and coarse thresholds chosen on the first 200 documents, then every suggestion for the rest, numbers
in their shortest round-trip form. Another implementation that follows docs/text-normalization.md should write the same
lines byte for byte. Set `GIL_WRITE_CORE_GOLDEN` to a directory to write both files afresh.

## License

Apache-2.0
