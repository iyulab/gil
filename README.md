# Gil

Suggests the decided attributes of a record — a ticket's team, a report's fault code — from the records already settled,
and says how often a suggestion it trusts is right. It needs no model: it learns from the values settled together and
from the settled records most like the one being filled, and how far to trust each is chosen by replaying the saved
records in the order they were settled. The library also carries an experimental path that answers requests in front
of an LLM from confirmed answers and one-token judgments; see [Stability](#stability).

**Status: early.** The library is being built from a specification whose behaviour was measured first in a
research harness. Published on NuGet as `Gil`, the decision core — native (Rust) inside, the package carrying its
library for each supported runtime — and, for the experimental path in front of an LLM, `Gil.Tree` with
`Gil.Abstractions` (`Gil.IronHive` and `Gil.Sqlite` stopped at 0.26.0); the API may still change within 0.x —
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
Measure it on your own saved records before relying on it — [Choosing what to trust](#choosing-what-to-trust)
says how.

It is not the right tool for an attribute whose value the other attributes do not decide (free text, a new name), for a form
nobody settles (memory then stays empty), or for choosing between models (a router does that).

## Layout

| Project | Contents |
|---|---|
| `Gil.Abstractions` | The model ports: `IChatModel` and `IEmbeddingModel` with their requests and results |
| `Gil` | The decision core, native (Rust) behind its C ABI: schemas, settled records, deciders, open records and their typed answers, threshold replay and tuning, usage counts, asking advice (`GIL0005`). It needs no other package |
| `Gil.Review` | Review statistics for checking automated decisions by hand: stratified samples, Neyman allocation, weighted estimates with intervals, agreement between reviewers — native (Rust) behind its C ABI. It needs no other package |
| `Gil.Tree` | Everything experimental (`GIL0001`–`GIL0004`): the tree judgment path — the tree YAML reader/writer, the single-token judge, the greedy traverser, output contracts, the fallback generator, slot filling, the resolver (memory → tree → narrowed fallback → full fallback), habits and their promotion, deactivation and differentiation, shadows — and what it is built on: the memories of confirmed answers, the call recorders and energy model, the telemetry contracts |
| `Gil.IronHive` | Chat and embedding models through any IronHive generator, with ready-made ones for OpenAI-compatible servers (retry rules for busy shared servers, the response kept as received). Optional: implement `IChatModel` and `IEmbeddingModel` yourself and `Gil` needs nothing else. No longer published: 0.26.0 is its last package, and the source stays here with the tree track |
| `Gil.Sqlite` | The telemetry store: every trace and model call, habit statistics, promotion and review logs in one SQLite file whose layout is the contract below. Optional: `Gil.Tree` takes any `ITelemetrySink`, and a consumer that keeps no telemetry ships no SQLite native library. No longer published: 0.26.0 is its last package, and the source stays here with the tree track |
| `Gil.Core.Tests` | Tests of the decision core through its .NET API, reproducing the Rust core's goldens |
| `Gil.Tests` | Unit tests of the experimental tracks and the compatibility fixture writer |

The `Gil` package carries the decision core's native library for eight runtimes, and the .NET host loads the one for
the platform it runs on:

| Runtime | Built for |
|---|---|
| `linux-x64`, `linux-arm64` | glibc 2.28 or later (the manylinux 2.28 baseline) |
| `linux-musl-x64`, `linux-musl-arm64` | musl 1.2 or later, such as Alpine 3.17 or later, with `libgcc` (which .NET needs there too) |
| `osx-x64`, `osx-arm64` | macOS 11 or later |
| `win-x64`, `win-arm64` | Windows, with the C runtime linked in (no Visual C++ redistributable needed) |

On any other runtime — 32-bit platforms, for one — the first call throws a
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
| `GIL0006` | The first record of a batch in the decision core: thresholds and promises chosen apart for a record whose batch has no settled record yet, and answering it at them |
| `GIL0007` | A new edition of the code lists in the decision core: a `Keeper` that starts again from nothing when a record arrives under a new `Record.FormVersion` |
| `GIL0008` | The least-cost rule in the decision core: `ThresholdRule.LeastCost`, the layers' `Calibration` it chooses with (`LayerThresholds.Calibration`, `AttributeDefinition.Calibration`), and answering by it |
| `GIL0009` | Value tails in the decision core: a layers selection's report, value by settled value, of how the chosen thresholds answered (`SelectionOptions.ValueTails`, `LayerThresholds.ValueTails`, `ValueTail`) |

Suppress the ids of what you use, in the project file or around the code:

```xml
<PropertyGroup>
  <NoWarn>$(NoWarn);GIL0001;GIL0003</NoWarn>
</PropertyGroup>
```

[The tree judgment path](#the-tree-judgment-path) describes the experimental path and walks through it: the
walkthrough needs `GIL0001`, `GIL0003` and `GIL0004`, and `GIL0002` for its habit promotion part.

## Usage

The decision core is the supported surface, and it needs no other package:

```sh
dotnet add package Gil
```

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
`Enter`, `Fill`, … — since memory and the replays treat them differently. Settled records are kept and read back as
JSON lines (`SettledRecords.FromJsonLines`), one line per settlement, in the form
[docs/settled-records.md](https://github.com/iyulab/gil/blob/main/docs/settled-records.md) specifies; an application can start keeping them before it uses Gil, and
the first thresholds are then chosen from what it kept. `decider.Answer(schema, id, values)` answers
from values alone and writes nothing; `AnswerPartial` answers one attribute while a person types into it.

When neither memory has evidence for an attribute, its answer's `NeedsExternal` lists the values a judgement outside the
core would read. `AnswerAsync` asks an `IExternalJudge` — a model, a rule engine, a service — for each of them and puts
what it ranks into the answers. `ResolverJudge` (`Gil.Tree`, `GIL0001`) is the [tree judgment path](#the-tree-judgment-path) as one:

```csharp
var judge = new ResolverJudge(resolver, new Dictionary<string, TaskDefinition> { ["team"] = task });
var judged = await ticket.AnswerAsync(decider, judge);
```

How far to trust each layer is chosen by replaying the settled records in settlement order, for the precision you
want. Put the choice on the schema, and keep it as a `Tuning`: at start-up, `Stale` says whether it still stands — the
same core version, the same settings it rests on, about as many records — and `Apply` puts it back.

```csharp
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
```

`WithLayers` puts the key threshold, the memory threshold and its similarity floor on the attribute, leaving a layer
with no chosen threshold unset (`WithCoarse` does the same for a coarse level). `Promise` records the precision each
chosen threshold promises for the next answers, and `PromiseOf` reads back the one an answer was made under, by the
layer it came from (a coarse class's is `CoarsePromiseOf`, recorded with `PromiseCoarse`). Settle with the answer that was in view and its promise, and the record keeps them — the value
suggested, its layer, its promise (`suggested:` for a value that was not the decider's): `Usage.Count` over the saved
records reports how each attribute's answers fared against the promise, and a reopened record carries them back until
the attribute is settled again:

```csharp
var promise = team.Answered ? tuning.PromiseOf("team", team.Source) : null;
// What was in view goes on the record it is settled on; the saved records are what the counts read.
ticket.Settle(decider, "team", SettlementKind.Accept, DateTimeOffset.UtcNow, value: "network", shown: team, promise: promise);
using var ledger = SettledRecords.From([ticket.Snapshot()]);
var counts = Usage.Count(ledger);
```

In the words operations reports use, for an attribute's `AttributeUsage`: the automation rate is `AnswerRate`
(`Answered` ÷ `Decisions`); the error rate of the answers is 1 − `Realized`; the acceptance rate is `Accepted` ÷
`Answered` and the override rate (`Corrected` + `Rejected`) ÷ `Answered`. Acceptance and overrides are read off answers
people saw, which they lean toward; a check counts answers no one saw at their weight, so `Realized` is the one to put
beside `Promise`.

While records keep arriving, a `Keeper` keeps that loop in one object. `Settle` puts records into the decider and into
the next choice, `Stale` says when the choice no longer stands, and `Rechoose` replays only the records settled since the
last choice — the same thresholds as `SelectLayers` on all of them, without replaying them all again:

```csharp
using var keeper = new Keeper(keyed, options);
keeper.Settle(decider, saved);
keeper.Rechoose();

// ... as records are settled ...
keeper.Settle(decider, settledNow);
if (keeper.Stale() is not null)
{
    keeper.Rechoose();
}

using var chosen = keeper.Schema; // answer with this and the decider; keeper.Tuning.PromiseOf gives the promise
```

It chooses for every decided attribute that takes one value, and for an attribute with a coarse level the level's
thresholds too, on the same replay — as `SelectCoarse` would with the value thresholds just chosen. A record settled out of order, or
one taken out with `Remove(decider, id)`, has the next choice replay from the start. Nothing is stored outside the
process: a keeper made at start-up replays the saved records once.

### Choosing what to trust

A threshold trades precision for coverage — the share of lookups answered, `AnswerRate`, the word selective prediction
and automated coding use. Gil holds the precision (the target, at the bottom of an interval) and reports the coverage
it reaches; "answered n times as often" below compares coverage at the same target.

The right thresholds move as memory grows. Choose them again from time to time — `Stale` says so once the settled
records have grown or shrunk by a tenth — by replaying the saved records. `SelectLayers` chooses both in one replay, in
the order the decider consults the layers: a similar record answers only where no key did, so its threshold is chosen
on those lookups alone. A layer without a `Chosen` threshold should not answer the attribute on its own; its
`MostPrecise` still says how close it came — the best precision it reached on at least `MinimumAnswered` answers — so
the application can say by how much the attribute falls short. Leaving an attribute unanswered is not a failure: it
records that what has been settled does not yet decide that attribute, so the person keeps deciding it. A layer starts
answering an attribute once a threshold is chosen for it, and not before.

`MinimumAnswered`, the fewest answers a threshold may be chosen on, is what keeps an early choice from resting on a
handful of lucky answers. Replaying 26 public coding tasks from their first records at a target of 0.8, 30 (the value
in these examples) kept every task that answered more than once at or above the target over its first 500 decisions,
where 20 let two fall below it; 50 also kept the one task that answered only once (and wrongly) from answering, at
about a quarter fewer answers overall (0.73 times as many). Do not set it below 30.


A replay's precision runs over every record it saw, so a layer that did better long ago keeps it high after the layer
has changed, and the threshold was chosen on those very answers, so their precision flatters it — most of all when they
are few. `ThresholdChoice.Promise` answers both. It takes the precision of the answers the threshold gives among the
latest `Replay.RecentRecords` (5,000) records (the whole replay's when those are too few to rest on) and lowers it to the
bottom of its Wilson score interval at one standard error for the answers it rests on, which `PromiseAnswered` reports.
On many answers the two hardly differ; on a few dozen the promise sits several points below. Show it as the promise —
it is what `Tuning.Promise` records. When `PromiseAnswered` is small, say so beside it: the number is a floor, not a
forecast. One standard error leaves about one chance in six that the precision the threshold holds is below it — a
one-sided 84% bound; `Pooled` holds its pair at 1.645, one-sided 95%.

Replaying three small public forms of about 670 records each, with thresholds chosen again whenever the records grew by
a tenth, the similar record layer's promise without that lowering ran 9.1 points above what its answers then
delivered, and 1.5 points above with it; the key layer's moved from between 0.1 below and 6.9 above to between 3.5
below and 1.6 above. On the first 5,000 records of two larger public streams, where the unlowered promise had already
been right, the lowered one came out 2.4 and 4.5 points below. Over long replays the recent window is what matters: on
two public streams of 80,000 records, the recent-window promise stayed within half a point of what its answers
delivered where the key layer's whole-replay precision ran 2.5 points high, and on a stretch where that precision had
drifted 9.6 points above the answers it was 2.7 points above. The window is 5,000 records, so on a stream shorter than
that the promise is read on the whole replay and trails a change: in a synthetic log stream of 3,000 records where one
template's grade changed at the 2,000th, the key layer promised 2.2 points more than the next 500 answers kept. Reading
it on the latter half of the replay instead turned that into 6.8 points too few on the 500 after, so the window stays
as it is; what moved first was the state under the changed key (below), whose precision there fell to 0.69 while the
attribute's promise still read 0.91. Neither mends a threshold chosen on the wrong lookups: a similar record threshold
chosen apart from the key layer promised 2.4–2.6 points more than it kept, which is why `SelectLayers` chooses both in
one replay. For the key layer, `ThresholdReplay.Keys` says, under each key that backed a best value, how many lookups it
had, how many the chosen threshold answered and how precise those were (weighted, as checks are), and whether it
answers under that key now (`KeyState`) — what an application checking its answers reads to decide how often to check
each key. A key reads as keys compare values — `template: t07` for a value settled as `T07` (see
docs/text-normalization.md). That policy stays the application's: a key answered rightly many times may be checked less
often, as long as the probability of each draw is still fixed before it.

Which target to ask for (`TargetPrecision`) is yours to set, and it follows from what a wrong answer costs. Count a
value a person enters with no answer shown as 1, a right answer as nothing, and a wrong one — noticed, undone and
entered again — as ρ. An answer right with probability p then costs ρ(1 − p) against the 1 of asking, so answering pays
only above a precision of 1 − 1/ρ: 0.5 where a wrong value costs two entries, 0.8 at five, 0.9 at ten, 0.95 at twenty.
Below that line a threshold that keeps its promise still loses — each answer it adds costs more than it saves.
Replaying a public stream of biological sample records with costs counted this way, thresholds chosen for 0.8 answered
right 0.88 of the time — a loss once a wrong value costs ten entries; chosen for a target at or above 1 − 1/ρ, they cost
less than asking at every ρ from 2 to 20. When you do not know ρ, time how long undoing a wrong value takes against
entering one.

A target can be read two ways. By default (`ThresholdRule.EveryBand`) each layer's threshold is the lowest down to
which every band of its answers meets the target, so no band of weak answers rides on good ones above it — a floor
under every kind of answer. When the target is a share of all the attribute's answers instead, ask for
`ThresholdRule.Pooled`: it chooses the two thresholds together, as the pair that answers the most of the attribute's
latest `Replay.PooledRecentLookups` lookups while the precision of all those answers taken together stays at or above
the target at the bottom of its Wilson interval at `Replay.PooledStandardErrors` (1.645) standard errors. Where the
attribute's thresholds as they stand would have answered fewer than `Replay.PooledRecentAnswers` of those lookups, it
reaches further back. In eight forward replays on seven public collections — medical device product codes, bug
components, aircraft fault codes, patent groups and the attributes of three kinds of biological samples — each on
records the rule was not tuned on, it kept a 0.8 target every time and answered 1.10–1.80 times as often as
`EveryBand` (1.55 in the middle); holding the bound at one standard error instead broke the target on an attribute
answered a few hundred times. Re-chosen by a `Keeper` as records arrived on two public streams at a 0.9 target,
`EveryBand`'s answers were 0.98 right — a floor under every band leaves the whole well above the target — and
`Pooled` answered 1.25 and 1.65 times as often, 0.92 right. Each layer's `Chosen` still reports that layer's own
precision and `Promise` (one standard error, its own answers); the bound the pair was chosen under is on the answers
together, and is the one to show:

```csharp
var pooled = Replay.SelectLayers(schema, "team", saved, options with { TargetPrecision = 0.8, Rule = ThresholdRule.Pooled });
```

When what you know is the cost of a wrong answer rather than a precision to keep, set the target where answering pays
— a wrong answer that costs ρ entries makes it `1 − 1/ρ` — and ask for `ThresholdRule.LeastCost` (experimental,
`GIL0008`). Each layer's best value gets a calibrated chance of being right, fitted on the latest `Replay.RecentRecords`
records' lookups; of the two layers the more likely answers, at or above the one chance threshold where answering costs
least, so the answers are right at least the target's share of the time on average. The thresholds come with that
calibration (`LayerThresholds.Calibration`); `Schema.WithLayers` puts it on the attribute, and the decider then lets
the more likely layer answer where both reach their thresholds. In replays on public recall records and public device event reports at targets of 0.5 and 0.75 — a wrong answer costing two and four entries — it cost 0.7–15.2 points less per decision than thresholds chosen layer by layer for the same cost; at other targets, and on the other collections tried, it was never dearer beyond chance. The first records of batches, chosen
apart, still answer in layer order.

The bound is on the answers together, not on any group of them. In a public bug tracker, on the bugs a deployed
component classifier had moved out of triage — harder than the rest — `Pooled` answered a quarter at 0.77 and
`EveryBand` a tenth at 0.90, against the classifier's 0.70. Split by who sent the records in — the recalling firm, the
bug's reporter, the maintenance report's submitter, the hospital — on four public collections, `Pooled` placed 19–22%
of its answers with senders whose own precision was below the target beyond doubt (the top of a 95% Wilson interval
under 0.8), and `EveryBand` none. `EveryBand` does not look at the sender, though: when one sender's own settlements are less
reliable than the others', it answers below the target for that sender too. If you read a promise per customer or per
site, choose `EveryBand`, and count `Usage.Count` over each sender's records to see it hold.

A value a person accepted with the answer in view may be right because it was answered. A value the application filled
was never checked. To know how right a layer is on its own, count only the settlements that saw no answer. Every record
is still replayed into memory; only the lookups counted change. A record that does not say how an attribute was settled
is counted only when `Counting` is left out:

```csharp
var unprompted = Replay.SelectLayers(schema, "team", saved, options with { Counting = [SettlementKind.Enter] });
```

Settlements that saw no answer are not a sample of the answers, though. Where the application shows every answer it has,
they are the records the thresholds did not answer, so a promise counted on them speaks of the records just below the
thresholds. Replaying two public streams that way, the thresholds stayed close to where counting every settlement put
them, but on one stream the promise fell from 0.98 to 0.90 while the answers were 0.98 right; on the other it moved
less than a point. To know how right the answers themselves are, have a few drawn at random settled again without the
answer in view (`SettlementKind.Check`, below). Counting the settlements that saw an answer leans the other way: if
people keep a wrong answer they see one time in three, a layer that is 0.90 right reads 0.93. What people keep is also
remembered, so a wrong value kept can come back as the next answer. Replaying two public streams with people keeping a
wrong answer one time in three, at a target of 0.9 and counting every settlement, the promise ended 0.3 and 0.8 points
above what was right, and about one remembered value in 200 was wrong: thresholds that keep the answers right
leave little that is wrong for people to keep.

A value settled as `Fill` stays in the record, but memory does not learn it and choosing thresholds does not ask about
it. Remembered, an answer's own value would come back as evidence for itself. Replaying a public stream with nine in ten
trusted answers filled, a replay that learned from them promised 0.97 where 0.73 was right, and one stream's key layer
ended up answering every record with 0.11 right. Learning only from what a person settled kept the promise within a few
points of what was right.

An application that fills values on its own still needs some of them checked, or nothing tells it when the fills go
wrong. Draw the decisions to check at random, with a probability fixed before the draw — the same for every decision
unless some keys need watching more closely — and have someone settle the attribute again without seeing the filled
value: `SettlementKind.Check`, with the probability. Memory learns the checked value in place of the filled one, and
choosing thresholds counts each check as one ÷ its probability decisions, the filled decisions it was drawn from, so
checking some keys more often does not tilt the promise toward them. Name `SettlementKind.Check` in `Counting` beside
the kinds a person settles. A decision picked because it looked doubtful has no such probability and is not a check:
counted as one, it would lean the promise toward the doubtful ones. Few checks weigh much but say little, so the fewest
answers a threshold must rest on is held to their effective number (`ThresholdChoice.EffectiveAnswered`).

```csharp
ticket.Settle(decider, "team", SettlementKind.Check, DateTimeOffset.UtcNow, value: "network", probability: 0.05);
var checkedLayers = Replay.SelectLayers(schema, "team", saved, options with { Counting = [SettlementKind.Enter, SettlementKind.Check] });
```

The check is a second settlement of the same attribute in the same open record, after the fill; the snapshot then keeps
the checked value, its kind and its probability (`Record.Probabilities`). Replaying two public streams with nine in ten
trusted answers filled and the tenth checked, counting the checks by their weight kept the promise within two points of
what was right in both, where counting them once had promised more than two points less than was right in one.
Weighting makes the promise honest rather than generous: where the answers had been doing worse than promised,
thresholds rose and about 8% fewer answers were right; where better, slightly more were. In a public bug tracker,
values a machine filled were changed by a person within 30 days 26% of the time, values a person chose 7%. Within the
same component the gap grows rather than shrinks — the machine's values were changed 4.8 times as often as people's
(3.5 to 6.7), so they are wrong more often, not merely placed where values move. Filled values need the checking.


Some values must not be missed: answering a dangerous grade as a harmless one costs far more than asking. A threshold
cannot hold such a value back where it is rare — a key under which it was settled once among hundreds of other values
still scores its usual value high. Name those values in `AttributeDefinition.MustCatch` (an attribute that takes one
value), and once one of them has been settled under a key the record has, no layer answers the attribute for that
record — neither the key layer nor a similar record, whose neighbours rarely include the value that is rare under the
key — and its coarse level is not offered. Its values under such a key are still offered, as guesses. Keyed by an
attribute drawn from many unrelated values (a host, a user), one such value settled under each soon holds back every
record, so key the attribute by what decides it (`KeyFields`, below). Replaying public system logs whose risk grade a
model had judged, naming grades 3 and above cut the lines graded 3 or more that were answered below 3 from 2 to 1 of
1,425, while keeping 81% of the lines the key layer answered. It reaches only keys where such a value was already
settled: the first one under a key is caught only if someone checks the answer, so it does not replace checking.

The replay looks each attribute up with the values that had arrived before it was settled — what its last answer was
made from. `Snapshot()` records that order in `Record.Arrival`; save it with the record. Without it, observed values
count as there from the start and decided attributes as settled in the schema's order. Even so, the precision a replay
reports can be optimistic, as the introduction warns. `TargetPrecision` is checked on the records the threshold is
chosen on. Where settled records come in batches — several at once from one source, alike in their observed values —
whether a near-identical record's value is right tends to hold or fail for the whole batch. The replay then sees fewer
independent cases than it counts. Key layer thresholds chosen once for a target of 0.8 on the first forty thousand
records of two public streams — an issue tracker and aircraft maintenance reports — answered right 0.85 and 0.90 of the
time on the forty thousand that followed, and 0.82 to 0.91 for the six attributes of a smaller stream of biological
sample records. Version 0.6.0, on two other streams of sample records, saw 0.63 to 0.74 at 0.8 and 0.64 to 0.72 at 0.7;
wider statistical margins did not close that gap. To know the precision an attribute actually delivers, check it on your
own records: choose on those settled up to a date and count the answers on those settled after it. Where you know which
records came in one batch, the optimism the batches add can be counted without waiting: name it in `Record.Batch`, and a
layers selection reports in `LayerThresholds.Batches` the precision of thresholds chosen on half of the latest batches
and counted on the other half (`AcrossBatches`), the same split by record (`AcrossRecords`), and how many answers a
batch holds and how much they are right or wrong together (`BatchSize`, `Correlation`, `DesignEffect`). On a month of
public medical device event reports, which manufacturers send many at a time, the precision of thresholds chosen for
0.8, counted on the records they were chosen on, was 1.9 points above what the reports that followed got; counted across
halves of the batches it was 0.1 points above, and across halves of the records, batch mates split between them, still
1.7. That is right on average only: the reports between two choices, a few batches each, came out anywhere from 0.64 to
0.89. Batches add optimism only when they are large and the records of one batch tend to be right or wrong together. On
two years of public food inspection records, where one inspection settles several different violations, they do not:
halves of the batches and halves of the records gave the same count, both 1.5 points above what followed, and the count
on the records chosen on was closest, 0.2 points above. On two years of public vehicle complaints, batched by model,
model year and week received, batch mates are right or wrong together about as much as the device reports are, but a
batch holds three complaints or so: halves of the batches and halves of the records again agreed within a tenth of a
point. Read `AcrossBatches` as the promise only where `AcrossRecords` stands above it and the design effect is well
above one; elsewhere the promise is the closer count.

A promise bounds all the answers its thresholds give; it says nothing apart about a batch's first record, answered
before any of its mates is settled, and `Batches` does not see it either. Where a batch's records come from one source at
once and show nearly the same things, the first one has nothing in memory to go on: on the device event reports its
answers fell below the promise by 5.0 and 2.6 points in two quarters, while on the food inspections and vehicle
complaints they held it. To hold first records to the target on their own (experimental, `GIL0006`), set
`SelectionOptions.FirstOfBatch` for the `Keeper` and open records with `OpenRecord.Open(schema, id, at, batch)`. The
keeper chooses the first records' thresholds on their own lookups and the rest on theirs, each resting on
`MinimumAnswered`; `KeeperChoice.FirstOfBatch` holds them, the keeper's schema carries them
(`AttributeDefinition.FirstOfBatch`), and an open record whose batch has no other settled record in the decider
(`IsFirstOfBatch`) is answered at them, its promise `Tuning.FirstOfBatchPromiseOf`. A record that names no batch always
counts as a first record, so with no batches named it answers as before.

When a form's code lists change meaning — a code renumbered, two merged, one split — a memory carried across answers
with codes whose meaning changed, and a promise chosen on the old edition does not hold on the new. Name the edition in
`Record.FormVersion` and set `SelectionOptions.RestartOnFormVersion` for the `Keeper` (experimental, `GIL0007`): a
record under a new edition starts the keeper again from nothing — the records before it leave the decider and the
replays, the thresholds chosen on them leave the schema, records still arriving under an edition left behind are not
taken, and `KeeperChoice.FormVersion` says which edition is kept. On public food inspections across a revision that
renumbered the violation codes, a memory carried over promised 0.94 for the first thousand records after it and was 0.44
right, and 120,000 records later still answered 0.52 of lookups against 0.81 for one started anew; on public injury
reports across an edition that changed no code, starting anew answered 0.63 times as many over the first five thousand.
Name a new edition only when a value comes to mean something else.

A promise is an average over an attribute's values, and a value the layers keep taking for another can sit far below
it while the average holds. Set `SelectionOptions.ValueTails` (experimental, `GIL0009`) for `Replay.SelectLayers` or the
`Keeper`, and the thresholds chosen carry `LayerThresholds.ValueTails`: for each value settled on at least
`Replay.ValueTailDecisions` of the latest records asked about, how many there were, the share the chosen thresholds
answered with another value (`AnsweredOtherwise`) and the value given in its place most often (`GivenInstead`). On public food inspections and public device event reports, the values whose share was 0.20 or more on the first half of the records were mostly still at 0.10 or more on the second (1.00 and 0.88 of them, against 0.10 and 0.06 of the other values).
A value with a large share is a candidate for `AttributeDefinition.MustCatch`. The report moves no threshold.

Settle a record that arrives again — a retransmission, a re-save — under the same record id. The memories keep one
version per id, and the replay asks a record settled again without its earlier version. Under a new id the copy is new
evidence: copies answer each other in the replay, and the threshold promises far more than other records get.
Replaying a public stream in which about half the records arrived twice, copies under new ids made the key layer answer
six times as often with 0.67 right against a promise of 0.91; under the same ids, it answered as if no copy had arrived.


The two layers answer different kinds of records. The key layer answers where a value the record already has decides
the attribute. The similar record layer answers the rest, which are harder. A schema whose decided attributes rest only
on observed attributes (`DependsOn` naming observed attributes alone) gets most of its answers from similar records. In
a replay of a public stream of settled records with the nearest record deciding alone, that layer's answers were right
about seven times in ten at most, even among the most similar records: records whose observed values read almost the
same had different settled values. Thresholds chosen there for a target of 0.8 delivered 0.66 to 0.72 on the records
that followed. Where settled records come in batches that share their observed values, expect the similar record layer
to fall short of a high target, and read even `MostPrecise` as optimistic: it is measured on the same records it was
chosen on.

The similar record layer lets the nearest `SimilarVotes` records vote — ten by default. Each votes for its settled
value with its similarity; the value with the most weight is the candidate, and its score is the vote's margin: the
winner's weight less the runner-up's, over all the weight cast, from 0 for a tie to 1 when every voter agrees. That
score is what `MemoryThreshold` is compared with, so a near record that its neighbours contradict is not trusted on its
similarity alone. Only records whose value lies in the attribute's domain vote. Replaying public streams of settled
records with thresholds chosen for a precision of 0.8, ten voters answered 1.8 times as many records right as the
nearest record alone in one stream and 1.2 times in another, at a higher precision (0.89 and 0.91 against 0.83 and
0.86). `SimilarVotes = 1` lets the nearest record decide alone on its similarity, as before 0.16.0. Choose
`MemoryThreshold` again after changing it.

The margin says how far the neighbours agree, not how closely they resemble the record being answered. A record written
in words no settled record uses still has nearest records — ones that share only an opening phrase or another
attribute — and they can agree. A threshold chosen by replaying records written the usual way never saw such a record,
and when that replay was right every time it chooses a threshold low enough to let them through. So the replay also
chooses a floor: of the answers at or above the threshold, the similarity of the nearest record voting for the answer
that only a hundredth of them fell below. Set it as `MemoryFloor` with the threshold (`SimilarityFloor` of the memory
layer's `Chosen`, as above); below it the vote is offered as a guess. In a synthetic stream where every settled record
used the same words for each topic, ten voters with the threshold alone answered about half of records reworded in
other words wrongly, and none with the floor. Replaying public streams with records stripped of their free text or of
its topic words, the floor cost under one percent of the answers to records written the usual way and answered no more
of the stripped ones wrongly. With a single voter the threshold already is a similarity, and no floor is chosen.

Which attributes key a decided attribute matters most when it has many values. An attribute that rarely decides it
still adds to the score of the values settled alongside its own: a value drawn from many, each seen only a few times —
a model number, say — lends a few wrong values most of their strength and pushes them above the right one. Name the
attributes that decide it in `KeyFields`. To find them in the saved records, replay the key layer with
`Replay.SelectKeyFields`, as above. It weighs a set of attributes by what its key layer gains at the target precision —
the right answers less the target times the answers, the amount the threshold itself is chosen to make largest — so an
answer counts only by how far it is right beyond the target. It tries each attribute the decided one rests on alone,
adds them in order of that gain while each addition gains more, and names a set only if it gains more than every
attribute together; otherwise `KeyFields` comes back null. Counting answers alone would drop an attribute that names
the very thing the record describes (a customer, a site, a piece of equipment): alone it answers only where that thing
comes again, so a broad attribute answers more often — and wrongly on exactly those. Then choose the thresholds again
with the attributes it named, since a narrower set changes the scores. Replaying a public stream of maintenance
reports, it chose two of thirteen observed attributes for a fault code with several hundred values: the key layer
answered 1.9 times as often as with every attribute, at a precision of 0.94 against 0.90. These were the two attributes
a learned weighting of the attributes ranked highest, and resting on them answered 2.2 times as often on the records
that followed, at 0.95 against 0.89. It costs one replay per attribute tried and one per addition.

`KeyFields` narrows the key layer only. The similar record layer still reads every attribute the decided one rests on
(`DependsOn`): matching a whole record discounts a line that does not decide the attribute rather than counting it, and
cut down to an attribute or two the lookups read alike. Replaying a public issue tracker with both layers on, resting a
component attribute on the one attribute the key layer chose (`DependsOn`) answered half as often; keying it by that
attribute (`KeyFields`) answered rightly 1.02 times as often as every attribute, on the same records at the same target.
The key layer took more of the answers and the similar record layer fewer, so precision moved from 0.89 to 0.88 —
above the target of 0.8, which is what the thresholds promise. On the maintenance reports above, with the similar
record layer and a coarse level on and a target of 0.95, keying the fault code by the one attribute chosen answered it
rightly 1.36 times as often as every attribute, at 0.96 either way; resting it on that attribute had answered less
often than every attribute.

How far a key seen only a few times is trusted depends on its attribute. A value's strength under a key is its weighted
count there over the key's weighted total plus a concentration, 1 unless set: a key seen once lends its value half its
strength, whatever attribute it comes from. An attribute that names the very thing the decided one describes — a
reference to another form's record, an equipment or customer number — keeps the value its keys were first seen with,
so a key of it seen once is as good as settled; a broad attribute whose keys mix values deserves less than half. Choose
a concentration for each attribute from the saved records with `Replay.SelectKeyConcentrations`. It replays them in the
order they were settled and asks, before each, how often a key seen before brought a value new to it; the
concentration under which that was likeliest is the attribute's. Two predictions from a key seen once are counted for
every attribute besides, so an attribute whose keys were seldom seen again stays near 1. Set them as
`KeyConcentrations`, after `KeyFields`, then choose the thresholds again, since the scores move:

```csharp
var concentrations = Replay.SelectKeyConcentrations(schema, "team", saved);
```

Replaying a year of public food inspections of one area (701 inspections, 382 establishments), where a risk level is
set per establishment and an inspection's kind is drawn from a few, the establishment attribute came out at 0.004 and
the kind at 0.23. Of the 319 inspections of an establishment inspected before, the key layer had answered 291 — 12
wrongly, following the kind against the establishment's own record — and abstained on 28; with the concentrations it
answered 313, all right, and abstained on 6. Establishments seen for the first time were answered as before (270 at
0.92, then 266 at 0.92). On a public issue tracker and the maintenance reports above, where no attribute names what is
decided, the concentrations ranged from 0.2 to 5 and the answers given rightly moved by −0.2% to +1.9%, every
threshold keeping its target.

A concentration near zero is also what an attribute gets when it was written from the decided value itself — a copy of
it, an identifier assigned to each value — and such an attribute may not be there yet while the record is being filled
in. With attributes like these planted beside the decided attribute of seven public streams of settled records, a copy
and an identifier per value came out below 0.001 in every stream, and none of the 141 attributes the records really had
came out below 0.14. Before trusting an attribute whose concentration is under 0.01, check that it is there when an
answer is asked for, as an establishment is; if it is filled only once the decided attribute is settled, save
`Record.Arrival`, so the replay looks the attribute up without it. An attribute computed from the decided value that
folds several values into one code reads to the replay as strong evidence and comes out like any other: only `Arrival`
keeps it out.


When another form has already settled the same subject — the same order, case or applicant — give its settled values
to this schema as observed attributes. The key layer learns from the settled records which of their values go with
which of this attribute's, and the similar record layer reads them as evidence; the two forms' values need not map one
to one. Replaying a public stream of patents classified under two schemes, with the class one scheme had settled given
as an observed attribute of the other, the attribute was answered rightly 1.8 times as often on the records that
followed, at the same precision (0.86); the same values shuffled between records answered no more often than without
them. Only a value the other form has settled is evidence: one still open there may change.

Where an attribute's values name their class in their leading characters — the chapter of a fault code, the class of a
patent classification, the group of a product code — a value the layers cannot answer can often still be given its
class. Set `AttributeDefinition.Coarse` to a `CoarseLevel` with the prefix length, and when no layer answers the value
`Answer.Coarse` offers a prefix: the one the values under the record's keys back by their summed scores, else the one
the similar records vote for. It is trusted at the level's own thresholds, chosen after the attribute's with
`Replay.SelectCoarse` on the lookups the value layers leave, and is a guess otherwise. It never enters `Candidates`,
which hold settled values only. The class is worth what follows from it — routing, a reviewer, a section of the form —
more than typing: a right two-character chapter saves one keystroke of four. Replaying public streams with thresholds
chosen on the first ten thousand records, the chapter of an aircraft fault code was trusted for 42% of the reports
whose code went unanswered at a target of 0.95, and was right for 97% of them; the class of a patent classification,
for 10% of the patents unanswered at 0.8, right for 85%.

Where a value's class is not in its characters but in a table — the panel that reviews a device code, the category of
a product — give the table instead, value to class. The values stay as they were settled and the classes are read off
them, so the table can change without settling anything again. A value the table does not list is a class of its own,
so list every value — those without a known class under a class named for them:

```csharp
var panel = new CoarseLevel(new Dictionary<string, string> { ["network"] = "infrastructure", ["payments"] = "finance" });
using var grouped = new Schema(schema.Name, schema.Attributes.Select(a => a.Name != "team" ? a : a with { Coarse = panel }));
var coarse = Replay.SelectCoarse(grouped, "team", saved, options);
```

Replaying public medical device recall records with the review panel of each product code from the public
classification table — 19 panels — the panel was trusted for 54% of the records whose code went unanswered at 0.8, and
was right for 90% of them; the code's own first letter, as a prefix, managed 9%.

A chosen threshold also holds only for the version of the core that chose it. A threshold rests on how a layer scores
its candidates and on what the replay asks, and a release may change either. A `Tuning` records the version that
chose it, and `Stale` returns `Stale.Version` under any other — choose again then. What an upgrade moves can be counted
before you take it. Replay your saved records once with each version — two small programs that differ only in the
version of the `Gil` package — choosing thresholds on the earlier records and asking for each later one before putting
it into memory, and write down each answer's first candidate, whether it was answered and its coarse class. Compare the
two lists record by record. A release marked **Breaking** changes a contract, not necessarily your answers: replaying
two public streams (medical device recalls and aircraft fault reports, ten thousand answers each after ten thousand to
choose from), 0.21.0, 0.22.0, 0.23.0 and 0.24.0 answered every one of the twenty thousand the same way. Choosing again
after an upgrade is still the rule; the count says what it will move.

When a person starts typing into a decided attribute, ask again with what has been typed rather than dropping the
answer: `AnswerPartial(decider, attribute, typed)` answers that attribute alone. Typing changes no other attribute's
answer, and where each attribute looks up similar records, answering the whole record again at every pause repeats
every lookup. The attribute stays open, and its answer offers only values that begin with the text (ignoring case;
Korean by keystrokes, so a syllable still being composed begins the values it leads to). Only the key layer answers
then, held to a threshold for that many characters typed, `TypedKeyThresholds`; a value trusted with fewer characters
typed, or before typing, stays trusted while the text still leads to it, so typing on never withdraws it. A person
types only where the answer before did not do, and the records left are the harder ones, so a threshold chosen on every
record promises more than it keeps there. Choose the typed thresholds with the key attributes: give `SelectKeyFields` a
`TypedLongest`, and each set of attributes is counted by its answers before typing and while typing up to that many
characters together — the attributes that decide an attribute before typing can blur it once a prefix has narrowed its
values — and `Typed` brings the thresholds for the choice, each length's chosen on the records typed that far that the
key threshold did not already answer rightly:

```csharp
var typing = Replay.SelectKeyFields(schema, "team", saved, options with { TypedLongest = 3 });
var typedThresholds = typing.Typed?.Thresholds;   // TypedKeyThresholds, beside KeyFields and KeyThreshold
var whileTyping = ticket.AnswerPartial(decider, "team", "ne");
```

Replaying a public stream of settled records with typed thresholds chosen on its first half, the first typed character
was where most of the gain lay: the decided attributes of the second half were answered rightly about thirteen times as
often as before typing, at a precision of 0.86 against a target of 0.8 — a value kept while typing on counts once, so
record feedback once per value, not once per question — and a person was spared 9% of the characters of the settled
values, against 1% before. A person who types on without looking saw no trusted value withdrawn. A screen that shows a
list rather than one value can instead keep the candidates that begin with the typed text: with twenty candidates,
filtering by the typed text saved about half of the keystrokes, against about a third when the list was shown once and
not filtered.

A decided attribute can take several values — tags, the topics a record covers, the answers to a multiple-choice
question. Mark it `Multiple`; its values are settled as a set (`SettlementKind.Set`), in `Record.Sets` rather than
`Values`, and order carries no meaning. Each value is remembered and answered on its own, so several candidates can each
be trusted against `KeyThreshold`, and the values a person has already chosen are evidence for the rest. An open record
keeps such an attribute open after it is settled and goes on answering the remaining values. The similar record layer
does not answer it, so it takes no `MemoryThreshold` and `SelectLayers` does not take it: its key threshold is the one
the key layer's replay in `Replay.SelectKeyFields` chose (`Chosen.Chosen?.Threshold`, with the key attributes it names),
which replays each record as values are picked one after another. A value that has a main one among several is better a
second, single-valued attribute that takes its candidates from the set (`CandidatesFrom`): once the record's values are
chosen, the main one is offered only among them. The same holds for any attribute whose values depend on another's,
such as a subcategory within its category; while the other attribute is empty, nothing is narrowed. Replaying a public
collection of patents, each classified into one or more of 37 technology classes, with one class of a record chosen,
using the chosen class as evidence put a remaining class first slightly more often (0.46 against 0.44) and trusted about
one and a half times as many of the remaining classes at the same precision (0.79 against a target of 0.8).

**Asking first** (experimental, `GIL0005`). When no layer answers a decided attribute, one more observed value from the
person — the component a ticket is about, the part a fault was found on — may let one answer, or a class be trusted.
Asking every time costs every one of those values typed. `AskPlanner` learns from settled records in settlement order
(`Learn`) what asking each attribute gained per character, and advises an open record within a budget (`Advise`).
Replaying a public stream of medical device recalls (asking for the recalling firm to answer the product code, ten
thousand records after thirty thousand) with a budget of 0.49, advice spent 49% of the typing and gained 0.032 per
record — 60% of what asking every time gained; on a public stream of aircraft fault reports (asking for the part to
answer the fault code), 48% and 0.094 — 68%.

## The tree judgment path

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

### Walkthrough

This walkthrough is the tree judgment path, which is experimental — see [Stability](#stability) for the ids to suppress.

```sh
dotnet add package Gil.Tree       # the tree judgment path this walkthrough uses, its memories and recorders
dotnet add package Gil.IronHive --version 0.26.0   # the models below (its last package); skip it if you bring your own IChatModel
dotnet add package Gil.Sqlite --version 0.26.0     # the telemetry store below (its last package); skip it if you keep no telemetry
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
(`.github/workflows/native.yml`) builds all eight and packs with them:

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

`cargo test` also checks `crates/gil-review` (`iyulab-gil-review`, imported as `gil_review`): the
statistics for checking decisions by hand — a seeded stratified sample, Neyman allocation, a stratified weighted
estimate with normal and bootstrap intervals, agreement between two reviewers. It depends on nothing, the decision core
included, and its golden is `tests/fixtures/review/golden.jsonl`, for a binding to reproduce.

Its .NET binding, `bindings/csharp/Gil.Review`, calls the crate through a flat C ABI
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
call. The plumbing both C ABIs share — statuses, the last error, the panic guard — is `crates/ffi-support` (`iyulab-ffi-support`).

Its tests are `tests/Gil.Core.Tests`, run with the rest of `Gil.slnx` as above.

## License

Apache-2.0
