# Changelog

Versions follow `0.x`: a minor release may change the public API. Each such change is listed under **Breaking**
with what to do. The Rust crate and Python package `iyulab-gil` keep their own line in
[crates/gil/CHANGELOG.md](crates/gil/CHANGELOG.md), and the review statistics (`iyulab-gil-review` and its .NET binding
`Gil.Review`) theirs in [crates/gil-review/CHANGELOG.md](crates/gil-review/CHANGELOG.md).

## Unreleased

### Added

- **Experimental (`GIL0007`): a keeper that starts again under a new edition of the code lists.**
  `SelectionOptions.RestartOnFormVersion` has the `Keeper` start from nothing when a record arrives under a new
  `Record.FormVersion`: the records settled before it leave the decider and the replays, the thresholds chosen on them
  leave the schema, records still arriving under an edition left behind are not taken, and `KeeperChoice.FormVersion`
  gives the edition kept. A record that names no edition is taken as of the current one, and the first edition named
  starts nothing.
- **Experimental (`GIL0006`): the first record of a batch, chosen and promised apart.** A promise taken on every
  answer holds for the records after a batch's first and can flatter the first, where a source sends records in
  batches that are right or wrong together. `SelectionOptions.FirstOfBatch` has the `Keeper` choose the value layers'
  thresholds for the first records of batches — no other settled record names the batch yet — on their own lookups and
  the rest on theirs, each resting on `MinimumAnswered`: `KeeperChoice.FirstOfBatch` gives them, the schema carries them
  (`AttributeDefinition.FirstOfBatch`, `Schema.WithFirstOfBatch`), and the tuning their promises
  (`AttributeTuning.FirstKeyPromise`, `FirstMemoryPromise`, `Tuning.PromiseFirstOfBatch`, `FirstOfBatchPromiseOf`).
  `OpenRecord.Open(schema, id, at, batch)` names a record's batch, its snapshot carries it, and a record that is the
  first of its batch (`OpenRecord.IsFirstOfBatch`, `Decider.IsFirstOfBatch`) is answered at the first records'
  thresholds (`Schema.ForFirstOfBatch`). A record that names no batch always counts as a first record, so with no
  batches named a keeper choosing apart chooses as before for every record.
- **`Keeper`: the promise-keeping loop as one object.** `new Keeper(schema, options)` holds the records settled and,
  for each decided attribute that takes one value, the replay as it stands. `Settle(decider, records)` puts records
  into the decider and into the next choice; `Stale()` says when the choice no longer stands (a tenth more or fewer
  records); `Rechoose()` replays only the records settled since the last choice, chooses each attribute's thresholds,
  puts them on `Schema` and captures `Tuning` with their promises — the same thresholds as `Replay.SelectLayers` on all
  the records, and for an attribute with a coarse level the level's thresholds as `Replay.SelectCoarse` would with the
  value thresholds just chosen (`KeeperChoice.Coarse`), on the same replay. A record settled out of order, or
  `Remove(decider, id)`, has the next choice replay from the start. The native library speaks version 5 of its C
  interface (this and the two above).
- **A coarse class has a promise of its own.** `Tuning.PromiseCoarse(attribute, thresholds)` records what a coarse
  selection's chosen thresholds promised (`AttributeTuning.CoarseKeyPromise` · `CoarseMemoryPromise`), and
  `CoarsePromiseOf(attribute, source)` reads it back by the coarse candidate's source; `PromiseOf` reads the value's
  layers only. A keeper records both.
- **musl-based Linux.** The `Gil` package also carries the native library for `linux-musl-x64` and
  `linux-musl-arm64` (musl 1.2 or later, such as Alpine 3.17 or later). There the .NET host used to fall back to the
  glibc build: on x64 that stopped at the first call with a `DllNotFoundException` (it needs `ld-linux-x86-64.so.2`),
  and on arm64 it loaded only because musl happens to provide what that build links against. `Gil.Tree`'s lexical
  memory runs on musl through the same library.
- **A settled record can say what was in view, how long it took and who settled it.** `Record.Suggested` (the value
  in view when a decided attribute was settled), `Sources` (the layer of the answer in view), `Promises` (the precision
  promised for it then, in [0, 1] — `Tuning.PromiseOf`), `SettleMs` (per attribute, the milliseconds from reaching it
  to settling it), `OpenMs` (the milliseconds the record was open), `SettledBy` (per attribute, an opaque id of who settled it) and
  `FormVersion` (the edition of the code lists, new when a value comes to mean something else) — the line format's seven new optional entries. The replays do not read them, and `Usage.Count` reads the first three; they are what a
  host keeps from its first record so that whether promises hold for the answers people saw, what settling costs and
  how settlers differ can be measured later. A promise out of [0, 1] is refused. Earlier versions refuse a line that
  carries them.
- **A selection's choice goes on the schema in one call, and an answer's promise reads from the tuning by its source.**
  `Schema.WithLayers(attribute, thresholds)` puts what `Replay.SelectLayers` chose on a decided attribute — the key
  threshold, the memory threshold and its similarity floor, a layer with no chosen threshold left unset — and
  `Schema.WithCoarse` does the same on the coarse level for `Replay.SelectCoarse`; both refuse a schema that would not
  check. `Tuning.PromiseOf(attribute, source)` gives the promise of the layer an answer came from, for
  `Record.Promises`. Setting the three properties with `with` and matching `KeyPromise` / `MemoryPromise` by hand
  still work. The native library speaks version 4 of its C interface.
- **What batches do to a promise.** `Record.Batch` names the batch a record was settled in — records settled together,
  several at once from one source. When any of the latest `Replay.RecentRecords` records names one,
  `LayerThresholds.Batches` (`BatchCheck`) gives the precision of thresholds chosen on half of them and counted on the
  other half, split by batch and split by record, over `Replay.BatchSplits` splits; how many answers a batch holds; how
  much they are right or wrong together; and the design effect. Thresholds are chosen as before, and records that name
  no batch read and write as before. On public records the two counts differed only where batches were large and right
  or wrong together (device event reports, design effect about 7: 1.6 points), not where batches were small (vehicle
  complaints, 1.5) or not right or wrong together (food inspections, 1.1).

- **What was in view goes on the record when it is settled.** `OpenRecord.Settle(…, shown: answer, promise: p)` keeps
  the answer's first candidate, its layer and `p` as the record's `Suggested`, `Sources` and `Promises`;
  `suggested: value` keeps a suggestion that was not the decider's. A promise out of [0, 1] is refused.

### Changed

- **`Gil.IronHive` and `Gil.Sqlite` are no longer published.** 0.26.0 is the last package of each; it depends on
  `Gil.Abstractions` / `Gil.Tree` 0.26.0 or later, so it keeps working beside this release. Nothing used either outside
  the experimental tree path; their source stays in the repository with that path, which is frozen.
- `SettledRecords.FromJsonLines` reads `settled_at` as a time in the round-trip format (`"O"`) as well as in ticks,
  and an entry given as null as left out — the line Python reads. A date, time or offset out of range does not read.
- A similar-record lookup over a large memory is split among up to eight threads; answers and replays are the same to
  the bit. Replaying 40,000 public food inspection records took 27 s against 60 s. `new Decider(threads: n)`,
  `SelectionOptions.Threads` and `Gil.Tree`'s `new LexicalMemory(threads: n)` cap them — 1 keeps a lookup on the
  caller's thread — for a host that answers many requests at once or keeps its own budget of threads.
### Breaking

- **`Usage.Count` counts the settled records, and `UsageDecision` is gone.** The counts read what a record keeps of
  what was in view — `Suggested`, `Sources`, `Promises` — beside how each decided attribute was settled: it was
  answered when the record names the layer the answer came from, or it was accepted or corrected; turned down when the
  answer was in view and nothing was settled; a check is judged against the suggestion kept. Put the answer in view on
  the record you save (`snapshot with { Suggested = …, Sources = …, Promises = … }`) and pass the saved records:
  `Usage.Count(SettledRecords.From(saved))` in place of `Usage.Count(decisions)`.

### Fixed

- **A reopened record keeps what the host kept on it.** `OpenRecord.Reopen` used to drop `Batch`, `Suggested`,
  `Sources`, `Promises`, `SettleMs`, `OpenMs`, `SettledBy` and `FormVersion`, so a record reopened to settle another
  attribute lost them from its next snapshot. They come back in each snapshot now, less an attribute's entries once
  that attribute is settled again.

### Documentation

- **How low `MinimumAnswered` may go.** Replaying 26 public coding tasks from their first records, 30 kept every task
  that answered more than once at its target over the first 500 decisions and 20 did not; 50 held one more edge case
  for 0.73 times the answers. The README now says not to set it below 30.
- **What a kept wrong answer does to memory.** Wrong answers people keep are remembered and can be answered again.
  Replaying two public streams with one wrong answer in three kept, at a target of 0.9 and counting every settlement,
  the promise ended 0.3 and 0.8 points above what was right and about one remembered value in 200 was wrong. The
  README says so beside how kept answers inflate a promise.
- **An unanswered attribute is a record, not a failure.** The README says, beside `MostPrecise`, that an attribute no
  layer answers is one the settled records do not yet decide, and that a layer answers an attribute only once a
  threshold is chosen for it.

## 0.26.0

### Breaking

- **Breaking: the decision core is native.** The `Gil` package is now the Rust decision core behind its C ABI, with
  typed .NET values on top; the package carries the native library of each supported runtime. Its vocabulary is the
  core's — schemas, attributes, records — and its answers, replays and tunings are the ones the Rust crate and the
  Python package `iyulab-gil` give, to the bit. The managed implementation is removed. Old type to new:

  | Before | Now |
  |---|---|
  | `FormDefinition` | `Schema` (`new Schema(name, attributes)`, `Find`, `FromJson`/`ToJson`; `IDisposable`): `Fields` is `Attributes`, `Field(name)` is `Find(name)`; `Keys`, `Supports`, `WithField` and `Language` are gone — build a new schema from changed attributes |
  | `FieldDefinition`, `FieldRole`, `FieldPolicy` | `AttributeDefinition` (`Attribute` in Rust and Python — the .NET name keeps clear of `System.Attribute`; settings as `init` properties, same names but `MemorySimilarityFloor` is `MemoryFloor` and `SimilarDocumentVotes` is `SimilarVotes`; `Admits` and `KeyThresholdFor` are gone — the core applies them), `AttributeRole` (`Observed`, and `Judged` is `Decided`), `AttributePolicy` (unchanged) |
  | `CoarseLevel` | `CoarseLevel` — `new CoarseLevel(prefix)` / `new CoarseLevel(classes)` and `Of(value)` as before, or set `Prefix` / `Classes` as `init` properties; `MemorySimilarityFloor` is `MemoryFloor` |
  | `SettledDocument` | `Record` (`DocumentId` is `Id`; `SettledAt` is a `DateTimeOffset`; `Validate(form)` is gone — a `Decider` refuses a record that does not fit its schema); a batch is `SettledRecords.From(records)` |
  | `Settlement`, `SettlementKind` | `OpenRecord.Settle(decider, attribute, kind, now, value, values, probability)`; `SettlementKind` unchanged |
  | `FormResolver`, `FieldMemory`, `LexicalMemory` (as document memory) | `Decider` (`Put`, `Remove`, `Answer`), which holds both memories |
  | `FormSession` | `OpenRecord` (`Open`, `Reopen`, `Observe`, `Settle`, `Answer`, `AnswerPartial`, `OfferExternal`, `Snapshot`) |
  | `FieldSuggestion` | `Answer` (`Field` is `Attribute`; with `Decision` when asked for); while typing, `PartialAnswer`. `SimilarDocuments` is gone: a candidate from the similar record layer names its nearest voter in `Evidence` (`RecordEvidence`: id and similarity), and the decision record's `Similar` holds the vote. `Confidence`, `Elapsed`, `Energy` and `TraceId` belonged to the model path and are gone |
  | `FieldCandidate`, `FieldSource` | `Candidate` (same members; `Evidence` is `KeyEvidence` or `RecordEvidence`), `Source` (`SettledFieldMemory` is `KeyMemory` and `SimilarDocument` is `SimilarRecord`, in JSON too — replace the two strings in anything stored; `Model` is `External`, and JSON still writes `Model`) |
  | `IFieldModel`, `FieldModelResult` | `IExternalJudge.JudgeAsync(attribute, evidence, ct)` returning ranked `ExternalCandidate`s (empty abstains), called by `OpenRecord.AnswerAsync(decider, judge, ct)` |
  | `ThresholdSelection` | `Replay` (`SelectLayers`, `SelectCoarse`, `SelectKeyFields`, `SelectKeyConcentrations`), synchronous, over `SettledRecords` and `SelectionOptions` |
  | `ThresholdSelection.RecentDocuments`, `PooledStandardErrors`, `PooledRecentLookups`, `PooledRecentAnswers` | `Replay.RecentRecords`, `Replay.PooledStandardErrors`, `Replay.PooledRecentLookups`, `Replay.PooledRecentAnswers` (same values) |
  | `ThresholdChoice`, `LayerThresholds`, `TypedKeyThresholds`, `KeyFieldsChoice`, `KeyFieldsTrial`, `KeyState`, `ThresholdRule` | Same names and members (a layers selection returns `LayerSelection`); chosen settings are kept as a `Tuning` (`Capture`, `Promise`, `Apply`, `Stale`, `ToJson`/`FromJson`). Members that were `init` properties are now positional — `ThresholdChoice` (`AnsweredWeight`, `EffectiveAnswered`, `SimilarityFloor`, `Promise`, `PromiseAnswered`), `ThresholdReplay` (`Keys`), `KeyState` (`AnsweredWeight`, `EffectiveAnswered`), `KeyFieldsChoice` and `KeyFieldsTrial` (`Typed`) — which matters only when building one by hand |
  | `AskPlanner`, `AskAdvice` (`GIL0005`) | `AskPlanner(schema, attribute, askable, budget)` with `Learn` and `Advise`, still `GIL0005` |
  | `GilDiagnostics` | Removed; answers carry their decision records, and `Usage.Count` reports how they fared |
  | `IEmbeddingPrefetch` | Removed |

- **The experimental types live in `Gil.Tree`.** The memories (`IMemory`, `MemoryMatch`, `LexicalMemory`,
  `EmbeddingMemory`, `CircuitBreakingMemory`, `MemoryReplay`, `MemoryUnavailableException`), the call recorders,
  `EnergyModel`, `EmbeddingGeneratorModel`, and the tree, judging, habit, review, promotion, shadow, telemetry and
  statistics contracts move from `Gil` and `Gil.Abstractions` into `Gil.Tree`. Their namespaces are unchanged: reference
  `Gil.Tree` instead. `Gil.Abstractions` keeps only the model ports (`IChatModel`, `IEmbeddingModel` and their requests
  and results).
  - The tree's `Candidate` is now `JudgeCandidate`, so it does not collide with the core's.
  - `IMemory`, `MemoryMatch`, `LexicalMemory` and `MemoryUnavailableException` are experimental (`GIL0004`);
    `PromptLanguage` is `GIL0001`; `EmbeddingGeneratorModel` is `GIL0003`.
  - `LexicalMemory` is the core's similar record memory, reached through the native library like the rest of the core,
    instead of a second implementation of it: the same similarities to the bit, ties and refusals. Lookups may now run
    from several threads at once (remembering and forgetting still run alone), and it is `IDisposable` — dispose it to
    free the native memory at once rather than at finalization.
  - `GilDiagnostics` is internal: listen to the activity source and meter `"Gil"`.
  - `ResolverFieldModel` is now `ResolverJudge`, an `IExternalJudge` (`GIL0001`): the request is the evidence as
    `name: value` lines, and the trace id and energy stay in the resolver's own sink.
  - `Gil.Tree` references `Gil`; `Gil` references none of `Gil.Tree`, `Gil.Abstractions`, YamlDotNet or
    Microsoft.Extensions.AI.

### Added

- A benchmark, `benchmarks/biosample/`, compares Gil with an association-rule value recommender on public NCBI
  BioSample records: scripts that build the stream from the NCBI dump and run both, and the results — how many values
  each answers at the same target precision on records that come later, with the recommender also learning as the
  test records settle, and whether each target is kept by the lower end of its interval.
- A record that keeps an attribute where its kind does not belong — a set for an attribute that takes one value, or
  one value for one that takes several — is refused with an `ArgumentException` naming the record and the attribute,
  wherever records or values reach the core (`Decider.Put`, `Decider.Answer`, `OpenRecord.Reopen`, the `Replay`
  selections, `AskPlanner.Learn`), and the decider goes on working. As `SettledDocument.Validate` refused it before.
- A call the core refuses no longer writes a panic note to the process's standard error: its message reaches you as the
  exception's, and nothing else. The same holds for `Gil.Review`.
- The `Gil` package carries the native library for `linux-x64`, `linux-arm64` (glibc 2.28 or later), `osx-x64`,
  `osx-arm64` (macOS 11 or later), `win-x64` and `win-arm64` under `runtimes/<rid>/native/`. On any other runtime the
  first call throws a `DllNotFoundException` naming the runtime and the supported ones; build the library with
  `cargo build --release -p gil-ffi` and put it next to the application.

### Documentation

- The figures for `Pooled` (eight forward replays, 0.8 target) are now those of the rule as published, which reaches
  back while the thresholds as they stand answer too few lookups: 1.10–1.80 times as many answers as `EveryBand`, 1.55
  in the middle. The figures given before, 0.92–1.80 and 1.52, were of a near variant that counted the answers each
  earlier choice had actually given, which differs only where answers are few; the target held and no attribute broke
  under either.
- The README's measured guidance on choosing a target and what to trust — choosing again as memory grows, the cost of
  a wrong answer, "A target can be read two ways", counting unprompted settlements and checks, batches and forward
  checking, repeated records, values that must not be missed, the promise's recent window, the similar record vote and
  its floor, what each layer answers, evidence from another form, key attributes and concentrations, coarse levels,
  typing, attributes that take several values, upgrades, asking first — is carried to the native API under "Choosing
  what to trust", with a new measurement: on four public collections `Pooled` placed 19–22% of its answers with senders
  whose own precision was below the target beyond doubt, and `EveryBand` none — and the note that `EveryBand` does not
  look at the sender: when one sender's own settlements are less reliable than the others', it answers below the
  target for that sender too, so count `Usage.Count` over each sender's answers to see a per-sender promise hold.
- The crate's README says which way the goldens run: the Rust core writes them and `Gil` is checked against them.
- The three READMEs name the trade a threshold makes in the words selective prediction and automated coding use:
  precision for coverage, the share of lookups answered (`AnswerRate`).
- The README says how to count, on records already settled, the optimism that batches add to a promise — choose on
  half of the batches and count on the other half — with a measurement on public medical device event reports: 1.9
  points counted on the records chosen on, 0.1 across halves of the batches — and on public food inspection records,
  whose batches are not right or wrong together, no difference, with the count on the records chosen on closest — and
  on public vehicle complaints, whose batches are right or wrong together but hold about three records, no difference
  either: batches add optimism only when they are large as well.
- The README's Usage now starts with the supported surface, the decision core (`dotnet add package Gil`), and the
  experimental tree judgment path follows under its own section with its walkthrough.
- `docs/settled-records.md` specifies the settled-record line every binding reads — its entries, why the time and
  the kind of settlement matter, and what an application should keep before it uses Gil — and the READMEs point to
  it.
- **When to ask a `Keeper` for `Pooled`.** At a 0.9 target on two public streams, a keeper choosing under
  `EveryBand` answered 0.98 right; under `Pooled` it answered 1.25 and 1.65 times as often, 0.92 right. The README
  gives the numbers beside the two ways to read a target.

### Dependencies

- `IronHive.Providers.OpenAI.Compatible` 0.66.1 → 0.68.0. Gil builds against it and its tests pass unchanged.

## 0.25.0

### Breaking

- `SettlementKind` has two more members, `AcceptOther` and `CorrectOther` (below): a `switch` over it that names every
  kind needs cases for them.

### Added

- **A value a person settled over a suggestion that was not the form resolver's.** An application may show its own
  suggestion — its own model's, a rule's, a default — in place of the resolver's. `Settlement.AcceptOther` (kept) and
  `Settlement.CorrectOther` (changed), recorded as `SettlementKind.AcceptOther` and `SettlementKind.CorrectOther`:
  memory learns the value and choosing thresholds counts it as it counts the others, but it says nothing about the
  resolver's suggestions. Settled as `Enter` such a value would count among the values settled with nothing in view, and
  as `Fill` it would be kept out of memory. The Rust crate and Python package `iyulab-gil` have had the same two kinds
  since 0.5.0.
- **`ThresholdRule.Pooled`: choose a field's two thresholds for a target on its answers together.** A new
  `ThresholdSelection.SelectLayersAsync` overload takes a `ThresholdRule`. `EveryBand`, the rule of every other
  overload, is unchanged. `Pooled` chooses the key and similar document thresholds as the pair that answers the most
  of the field's latest `PooledRecentLookups` lookups while the precision of all those answers holds the target at the
  bottom of its Wilson interval at `PooledStandardErrors` (1.645) standard errors, reaching back while the field's
  current thresholds answered fewer than `PooledRecentAnswers` of them. Some bands of its answers fall below the target
  where others are above it. See "A target can be read two ways" in the README. The Rust crate and Python package
  `iyulab-gil` have the same rule (`replay::select_layers_by_rule`, `select_layers(..., rule="pooled")`).

### Documentation

- The repository is https://github.com/iyulab/gil again; `iyulab/gilfit` redirects. The NuGet package ID and
  namespace stay `Gil`. The Rust crate and the Python package are published as `iyulab-gil` from 0.3.0 (imported as
  `gil`); `gilfit` 0.1.0 and 0.2.0 are yanked.
- README: how to choose `targetPrecision` from what a wrong answer costs — answering pays only above `1 − 1/ρ`
  when a wrong value costs ρ entries.
- README: a key concentration under 0.01 can mean a field written from the judged value; check that it is there when
  a suggestion is asked for, or save `Arrival`.
- README: values a machine filled are changed more often than people's within the same component too (4.8 times
  in a public bug tracker), not only overall.

## 0.24.0

### Breaking

- **Typed text and prefixes count Unicode scalar values, and case is compared by the simple lowercase mapping.** How
  much is typed (the entry of `FieldDefinition.TypedKeyThresholds`), a coarse level's `Prefix`, and the cut of a value
  replayed as typed were counted in UTF-16 code units, so a character beyond U+FFFF counted twice and a prefix could
  end inside one; whether a value begins with typed text was an ordinal case-insensitive comparison. Now each counts
  scalar values and compares them one for one by the lowercase mapping keys already use (U+0130 meets `i`), as
  `docs/text-normalization.md` now specifies under "Typed text and prefixes". Text inside the Basic Multilingual Plane
  without such case pairs is unchanged; choose typed thresholds and coarse thresholds again where values hold
  characters beyond it.

### Added

- **Experimental (`GIL0005`): `AskPlanner`, which observed field to ask a person for first.** For a judged field and the
  observed fields a person could be asked for, it learns from settled documents, in settlement order, what asking each
  field gained per character typed: the judged field's suggestion without the field against the suggestion with it,
  under the coarse class the suggestion without it showed. `Advise` ranks an open document's empty fields by that gain
  and marks those worth asking within a budget — a share of the typing that asking every time would take. Nothing
  changes for code that does not use it. See "Asking for a field first" in the README.

### Fixed

- `LexicalMemory` squares the weights of a query's norm by multiplying rather than with `Math.Pow(x, 2)`, which is not
  correctly rounded on every platform: the same documents could score a similarity one or a few units in the last
  place apart from one operating system to another. Similarities are now the same everywhere; where they moved, they
  moved by that much. The Rust rebuild squares the same way, so a release build no longer folds
  `powf(2.0)` into a product where the reference called the platform's `pow`.

### Documentation

- How to count what an upgrade changes in your own suggestions: replay saved documents once with each version and
  compare each suggestion's first candidate, whether it was answered and its coarse class (README, under choosing
  thresholds again). Measured on two public streams, 0.21.0 through 0.24.0 answered the same.

## 0.23.0

### Breaking

- **A field that takes one value has at most one trusted candidate from the key layer.** When the key layer answered,
  every value it ranked under the document's keys was marked `Trusted`, so one suggestion could carry three trusted
  values that disagree. Now only the value it answers with is trusted; the values after it are guesses, as
  `FieldCandidate.Trusted` says a guess is. `FieldSuggestion.Answered` and the first candidate are unchanged. An
  application that listed every trusted candidate as a suggestion now lists the answer, and a trusted value from the
  similar document layer, if any, comes right after it. Fields that take several values still trust each value on its
  own score.

### Added

- `FormDefinition.WithField` puts a field with new settings back into its form, in the same place, checked as a new form
  is — the step after choosing a field's thresholds or keys, which the README showed only up to `form.Field(...) with`.

### Fixed

- **A value to catch (`FieldDefinition.MustCatch`) is no longer answered by a similar document.** Once such a value
  had been settled under a key the document has, the key layer held back, but the similar document layer then answered
  in its place — and the neighbours that agree on a field's usual value rarely include the value that is rare under the
  key. Now no layer answers the field for such a document and its coarse level is not offered; its values are still
  offered as guesses. In a synthetic log stream filled automatically with 5% re-checked, the dangerous value was answered
  as another value 7 times out of 8 before and 0 times after, all 8 held for a person. `SelectLayersAsync` and
  `SelectCoarseAsync` no longer leave such lookups to the similar document layer or the coarse level: choose those
  thresholds again where `MustCatch` is set. A field keyed by every other field catches under each of them, so one such
  value under each value of a broad field (a host) holds back the whole field — in the same stream answers fell from
  1,948 to 532; keyed by the field that decides it, they stayed at 1,911.

### Dependencies

- `IronHive.Providers.OpenAI.Compatible` 0.57.0 → 0.66.1. None of its breaking changes touches the calls Gil makes; an
  HTTP 402 from the server now surfaces as IronHive's `BillingException`. Its packages now pin one another to the same
  version, so an application that also references another IronHive package moves it to 0.66.1 too.
- `Microsoft.Data.Sqlite` 10.0.12 → 10.0.13 · `Microsoft.Extensions.AI.Abstractions` and `Microsoft.Extensions.AI.OpenAI`
  10.10.1 → 10.11.0.

### Documentation

- The README shows the path of an application that fills values on its own: fill a trusted answer, check a share drawn
  with a fixed probability by settling the field again in the same session, and count the checks when choosing
  thresholds. It also says how `KeyState.Key` reads (the value normalized as keys compare it).
- The README says how long the recent window behind `ThresholdChoice.Promise` is (5,000 documents), that a shorter
  replay is read whole so its promise trails a change, and that the state under each key shows the change sooner. A
  window of half the replay was measured and kept out: on a short synthetic stream it traded 2.2 points of optimism
  after a change for 6.8 points of pessimism after the next choice.

## 0.22.0

### Breaking

- **`SelectKeyFields` weighs a set of fields by its gain over the target, not by its answers.** A set's key layer is now
  scored by its right answers less the target precision times its answers (weighted, typed lengths included) — what
  the threshold itself is chosen to make largest. Counting answers let a broad field win over a field that names the
  very thing judged: alone the latter answers only where that thing comes again, and the broad field's extra answers
  near the target were the wrong ones there. On a small public form with such a reference field, keyed by the choice
  after `SelectKeyConcentrations`, the wrong answers on things that came again went from 11 to 0 and the chosen fields
  stopped switching between choices (8 switches to 0); over 23 fields of eight other small public forms the gain rose
  on 13 and fell on 3, and three large public streams chose the same fields as before. A form may now be keyed by
  different fields: choose its key fields and thresholds again.

## 0.21.0

### Breaking

- **`ThresholdChoice.Promise` is the low end of what the replay supports.** It was the precision of the answers the
  threshold gives among the latest documents; it is now that precision lowered to the bottom of its Wilson score interval
  at one standard error for `PromiseAnswered` answers. The threshold is chosen on those same answers, so their precision
  flattered it most when they were few: on three small public forms (about 670 records each) the similar document
  layer's promise ran 9.1 points above the answers that followed, and runs 1.5 points above now. On many answers it
  barely moves; where the old promise was already right it now sits 2–5 points below. Thresholds, `Precision` and every
  suggestion are unchanged — only the number shown as the promise is lower, by most where the answers are few.

### Added

- **How readily each key field's keys are believed, chosen from settled documents.** A value's strength under a key was
  its weighted count over the key's weighted total plus 1 for every field, so a key seen once lent half its strength
  even where its field names what is judged — a reference to another form's document, an equipment number — and a broad
  field whose keys mix values outvoted it. `FieldDefinition.KeyConcentrations` sets the term per key field (1 unless
  set, which scores exactly as before), and `ThresholdSelection.SelectKeyConcentrations` chooses it by replaying the
  documents in settlement order: each field takes the concentration under which its keys' predictions of the next value
  were likeliest. Choose the thresholds again after setting it (README "Filling forms").

### Documentation

- A golden for the form path under `tests/fixtures/core`: made-up settled documents and what this implementation decides
  from them, for another implementation to reproduce byte for byte (README "Build and test"). It includes the key
  concentrations chosen for one field.

## 0.20.0

### Breaking

- **The tree judgment path is its own package, `Gil.Tree`.** `Resolver`, the tree and its YAML, traversal, one-token
  judging, fallback generation and slot filling, habits and their promotion, review, deactivation, differentiation and
  shadows, and `ResolverFieldModel` moved out of `Gil` into `Gil.Tree`, with the same namespaces and the same
  experimental ids. Code that uses them adds `Gil.Tree`; `Gil.Sqlite` now depends on it. `Gil` no longer depends on
  YamlDotNet, and a form consumer changes nothing. The `gil.resolve` span and the `gil.resolution*` metrics come from
  `Gil.Tree` under the same name (`GilDiagnostics.Name`).
- **The form path takes one kind of document memory, `LexicalMemory`.** `FormResolver`'s `documentMemory` and the
  `memory` of `ThresholdSelection.SelectAsync`, `SelectLayersAsync` and `SelectCoarseAsync` are `LexicalMemory` instead
  of any `IMemory`. Code that passes a `LexicalMemory` (typed as one) or `null` compiles as before. `EmbeddingMemory`,
  `CircuitBreakingMemory` and a memory of your own still fit through overloads that take any `IMemory`, now marked
  experimental with the new id `GIL0004`: suppress it to keep using them (`<NoWarn>$(NoWarn);GIL0004</NoWarn>`).

### Changed

- **`SelectCoarseAsync` takes no document memory when the form resolver has none.** Pass `memory: null`: the class the
  keys back is chosen as before, nothing votes, and `LayerThresholds.Memory` is a replay with no answers. Before, the
  call required a memory, so a consumer without one replayed a lexical memory it would never use.
- **Text is normalized, cut into n-grams and ordered by Unicode scalar value**, as docs/text-normalization.md now
  specifies, so another implementation can reach the same keys, n-grams and sums. Lower-casing is the simple mapping of
  each scalar (U+0130 now becomes i), `LexicalMemory` counts n-grams of scalars (a character beyond U+FFFF is no longer
  two halves), a key's length counts scalars, ties and the values of a set sort by scalar value, n-gram ids and every
  sum over them follow the n-grams' order, and the hash that orders a set's values when choosing thresholds is FNV-1a
  over UTF-8. Other text is normalized as before; replaying three public streams (4,000 documents each), the
  suggestions and chosen thresholds were the same, with scores differing in the last digits. Choosing thresholds again
  after upgrading is not needed for that, but a set-valued field's choice may move slightly, as its values are asked
  about in a new order.
- README says how to leave a field empty: leave it out of the document; an empty string is learned as a value.
- The repository moved to https://github.com/iyulab/gilfit; the old address redirects. The package and project
  links, the security advisory link and the diagnostic help links for the experimental surface point there.

## 0.19.0

### Breaking

- **The experimental surface is marked, and using it needs its diagnostic id suppressed.** The library now says which
  part is supported — filling forms: `FormResolver`, `FormSession`, `FieldMemory`, `ThresholdSelection`, the form
  definitions and settlements, the memories, the chat and embedding models — and marks the rest `[Experimental]`:
  `GIL0001` the tree judgment path (`Resolver`, task and tree definitions, `TreeYaml`, traversal, one-token judging,
  output contracts and fallback generation, `ResolverFieldModel`), `GIL0002` habit promotion (proposals and review,
  deactivation, differentiation, shadow evidence), `GIL0003` recording and pricing (`ITelemetrySink`,
  `SqliteTelemetryStore`, `CallRecorder`, `EmbeddingRecorder`, `EnergyModel`, statistics, `MemoryReplay` and rebuilding
  a memory from the feedback log). A marked type may change or go in any release without a migration note. Code that
  uses one stops compiling until the id is suppressed: add `<NoWarn>$(NoWarn);GIL0001;GIL0003</NoWarn>` (the ids you use)
  to the project, or `#pragma warning disable` around the use. A form consumer needs none, with one case to change:
  `new EmbeddingMemory(new EmbeddingRecorder(model, energy))` uses a marked type — write `new EmbeddingMemory(model)`
  if the embedding calls need no recording or pricing, or suppress `GIL0003` to keep the recorder. README "Stability"
  lists what each id covers.
- **The tree is called a tree: `Gil.Ontology` is `Gil.Tree`.** `OntologyYaml` is `TreeYaml`, `LoadedOntology` is
  `LoadedTree`, `OntologyException` is `TreeException`, and `TaskDefinition.Ontology` is `TaskDefinition.Tree`. The
  type reads and writes a decision tree, which the README already called one. Rename the four; the YAML itself — its
  keys and files — is unchanged.

### Added

- `EmbeddingMemory(IEmbeddingModel model, int pendingLimit = 10_000)` — an embedding memory straight from a model, for
  a consumer that neither records nor prices its embedding calls. Lookups and remembering report an energy of zero.
- **Checking filled values.** `Settlement.Check(value, probability)` (`SettlementKind.Check`) settles a value the
  application filled again, by someone who did not see it, the decision having been drawn to be checked with
  `probability` — fixed before the draw. Memory learns the checked value, the snapshot keeps the probability in
  `SettledDocument.Probabilities` (save it with the document), and choosing thresholds counts a check as one ÷ its
  probability decisions: the decisions it was drawn from. Precision and answer rate are then weighted, and the minimum
  number of answers is held to the effective number — `ThresholdChoice.EffectiveAnswered`, the squared sum of the weights
  over the sum of their squares — beside `ThresholdChoice.AnsweredWeight`. With no check every choice is what it was.
  Choosing key fields ranks them by the weighted answers. Name `SettlementKind.Check` in `counting` to count checks
  beside what a person settled.
- **Values a field must not miss.** `FieldDefinition.MustCatch` names values — a dangerous grade, say — that answering
  as something else costs far more than asking. Once one of them has been settled under a key the document has, the key
  layer does not answer the field for that document (its values are still offered, untrusted), and choosing key
  thresholds asks such lookups without counting them as the key layer's. Only for a judged field with a single value.
- **The promise over the latest documents.** `ThresholdChoice.Promise` is the precision of the threshold's answers among
  the latest `ThresholdSelection.RecentDocuments` (5,000) documents replayed — the whole replay's `Precision` when those
  answers are worth fewer than the minimum — with `PromiseAnswered`, how many answers it rests on. Show it as the promise
  for thresholds chosen layer by layer (`SelectLayersAsync`, or `SelectKeyThreshold` with the key layer alone):
  `Precision` still runs over every document and keeps a better past. README has what was measured.
- **State under each key.** `ThresholdReplay.Keys` lists, for the key layer of a single-valued field, each key that
  backed a best value (`KeyState`): its lookups, the answers the chosen threshold gave under it, their weighted
  precision, and whether its latest lookup was answered. Null for the other layers. A lookup held back by `MustCatch` is the key's
  but never answered. `ThresholdReplay` now holds a list, so two replays compare equal only by their contents
  (`Keys` compares by reference).
- **A coarse level from a table.** `new CoarseLevel(classes)` takes value-to-class pairs for a field whose classes are not
  prefixes of its values; `CoarseLevel.Classes` keeps the table and `Prefix` is 0. Suggestions and
  `ThresholdSelection.SelectCoarseAsync` group and judge by `CoarseLevel.Of`, which reads the table — a value it does not
  list is a class of its own. Before, such a field had to store its values with the class in front.

### Documentation

- README: what makes a key — a consumer-supplied short field, and values longer than the key limit are not keys.
- README leads with filling forms; the tree judgment path moves under "Stability" with the experimental ids.
- README: what thresholds chosen once delivered afterwards, measured again with this version; the earlier figures stay
  with the version that measured them.

## 0.18.0

### Breaking

- **A suggestion while typing is asked for the field typed into alone.** `FormResolver.SuggestAsync(form, documentId,
  values, sets, typed)` and `FormSession.SuggestAsync(typed)` suggested every open field at each pause; typing changes
  no other field's suggestion, and where fields look up similar documents each pause repeated every lookup — a form
  with five such fields and 100,000 saved documents took about 0.2 s a pause. Call
  `FormResolver.SuggestAsync(form, documentId, field, values, typed: text)` or `FormSession.SuggestAsync(field, text)`
  instead; both return the one field's `FieldSuggestion`, and keep the other fields' suggestions as last shown.
- **Narrowing what a field rests on is for its key layer alone: `SelectDependsOn` is now `SelectKeyFields`.** The
  fields it chose went into `DependsOn`, which also decides what a similar document is looked up by. Its choice was made
  on the key layer — where a many-valued field that rarely decides the field pushes wrong values up — but the similar
  document layer matches every line it is given and discounts the ones that do not decide; narrowed to a field or two,
  its lookups read alike and it stopped answering. Replaying a public issue tracker with both layers on, resting a
  component field on the one field chosen halved its answers. `FieldDefinition.KeyFields` now names the fields that key
  a field in the settled field memory, within the fields it rests on; the similar document layer and a model still read
  every field it rests on. `ThresholdSelection.SelectKeyFields` returns `KeyFieldsChoice.KeyFields` (`KeyFieldsTrial`
  for each set tried), and the field's own `DependsOn` now bounds the fields it tries. Replace
  `field with { DependsOn = narrowed.DependsOn }` with `field with { KeyFields = narrowed.KeyFields }` and keep
  `DependsOn` as it was; then rebuild the document memory and choose the thresholds again. The key layer replays exactly
  as it did resting on the same fields.

### Added

- **One field's suggestion on its own.** `FormResolver.SuggestAsync(form, documentId, field, values, sets, typed)` and
  `FormSession.SuggestAsync(field, typed)` suggest one open judged field as the whole form's suggestion would, making
  the similar document lookup and the model call for that field only. An application that shows one field at a time
  no longer pays for the rest. A field that is not judged, is off, or already has a value is refused.
- **How a value was settled is kept with the document.** Two settlement kinds join `Accept` and `Correct`, which
  mean a person had the suggestion in view: `Settlement.Enter` (a person settled the field with no suggestion shown)
  and `Settlement.Fill` (the application settled the suggested value on its own). `SettledDocument.Settlements` keeps
  the kind of each judged value, and `FormSession.Snapshot()` fills it. A restored value, and a document saved before
  this release, is not known to have been settled any way. A filled value stays in the document, but memory does not
  learn it and choosing thresholds does not ask about it: a suggestion's own answer remembered would come back as
  evidence for itself.
- **Choosing thresholds can count only some settlements.** Every `ThresholdSelection` method takes
  `counting`, the settlement kinds whose lookups count. For example, `[SettlementKind.Enter]` measures a layer on values
  settled with no suggestion in view. Every document is still replayed into memory. Left out, every settlement counts,
  as before.
- **A saved document reopens as it was saved.** `FormResolver.Open(form, saved)` brings back a snapshot's values, the
  order they arrived in, its time and how each value was settled. Putting values back one by one with
  `Settlement.Restore` took the order they were put back in as the order they arrived in, so a replay of a document
  saved after a reopening in another order asked about its fields with other evidence than its suggestions had.

### Dependencies

- `IronHive.Providers.OpenAI.Compatible` 0.50.0 → 0.57.0 (`Gil.IronHive`). Nothing `Gil.IronHive` uses changed shape;
  a template switch given in `ExtraBody` is still sent as given, and no thinking budget is added to Gil's requests (Gil sets
  no reasoning effort, so 0.57.0's fitting of the thinking budget inside `MaxTokens` does not apply).

## 0.17.0

### Added

- **A coarse level for values that name their class in their leading characters.** Set `FieldDefinition.Coarse` to a
  `CoarseLevel(prefix)` and, when no layer answers the value, `FieldSuggestion.Coarse` offers the class: the prefix the
  values under the document's keys back by their summed scores, else the one the similar documents vote for — trusted
  at the level's `KeyThreshold`, or `MemoryThreshold` with its `MemorySimilarityFloor`, a guess otherwise. Choose them
  with `ThresholdSelection.SelectCoarseAsync` after the field's own thresholds. `Candidates` still hold settled values
  only. Replaying public streams, the class was trusted for 42% and 10% of the values left unanswered, right 97% and 85%
  of the time.

### Fixed

- **A vote among documents unlike the draft is no longer trusted on their agreement alone.** The similar document
  layer's score is how far its voters agree, and a draft written in words no settled document uses still has nearest
  documents — sharing only an opening phrase or another field — that can agree. A threshold chosen on drafts written
  the usual way promised nothing for such drafts, yet trusted them; where the replay was right every time, it trusted
  half of them, wrongly. Choosing a memory threshold now also chooses a similarity floor
  (`ThresholdChoice.SimilarityFloor`): of the answers at or above the threshold, the similarity of the nearest voter for
  the answer that only a hundredth of them fell below. **Set it as `FieldDefinition.MemorySimilarityFloor` with
  `MemoryThreshold`**; below it the voted value is offered as a guess. Thresholds themselves are unchanged. Replaying
  public streams, the floor cost under one percent of the answers to drafts written the usual way and answered no more
  drafts stripped of their free text wrongly. A field with `SimilarDocumentVotes = 1`, and a lookup with a single voter,
  have no floor: the score is then the similarity.

## 0.16.0

### Breaking

- **The similar document layer votes.** The nearest `FieldDefinition.SimilarDocumentVotes` settled documents — ten by
  default — vote for their values, each with its similarity; the value with the most weight is the candidate, its
  evidence the nearest document that voted for it, and its score the vote's margin (the winner's weight less the
  runner-up's, over all the weight cast). The score was the nearest document's similarity. `MemoryThreshold`,
  `FieldCandidate.Score` and the trace's `Recall.Similarity` for that layer now carry the margin, so **choose
  `MemoryThreshold` again** (`ThresholdSelection.SelectAsync` or `SelectLayersAsync`, which replay the same vote).
  Replaying public streams with thresholds chosen for 0.8, the vote answered 1.8 and 1.2 times as many documents right
  at a higher precision. Set `SimilarDocumentVotes = 1` to keep the nearest document deciding alone.
- **Documents outside a field's domain do not vote.** The layer used to offer nothing when the nearest document's value
  lay outside `Candidates` or `CandidatesFrom`; the documents inside it now vote — with `SimilarDocumentVotes = 1`, the
  nearest of them decides. A memory that ranks only its nearest document (the default `IMemory.NearestAsync`) still
  offers nothing then.

## 0.15.0

### Added

- **`SelectDependsOn` can count typing.** The fields that decide a field before typing can blur it once a person has
  typed a prefix, so a set chosen by answers before typing alone can cost answers while typing. Give `typedLongest`
  and each set is counted by its answers before and while typing together; the typed thresholds for the choice come
  with it (`DependsOnChoice.Typed`, and `DependsOnTrial.Typed` for each set tried). Without it the choice is as before.

## 0.14.0

### Added

- **Korean typed text matches the syllable still being composed.** Hangul is typed a letter at a time, so the text at
  a pause may end in a syllable on its way to another — "바" or "ㅂ" on the way to "박물관", "박" on the way to "바가지",
  "오" to "와". Typed text with Hangul now begins a value when its keystrokes begin the value's (compound vowels and
  finals as two keystrokes, doubled consonants as one). Thresholds still count characters typed. Text without Hangul is
  compared as before.

### Changed

- **A value trusted while a person types stays trusted as they type on.** Typed text only narrows the values, so a
  value trusted with fewer characters typed — or before typing, under `KeyThreshold` — is still the best value, with the
  same score, while the text leads to it. It was judged again by the threshold for each length, and where that was null
  (past `TypedKeyThresholds`, or not chosen) the suggestion was withdrawn mid-word. It now stays trusted; other values
  are held to the threshold for their length as before. `SelectTypedKeyThresholds` leaves out of each length the
  documents a shorter length trusted a wrong value on that the typed text still leads to, so its thresholds can differ:
  choose them again after upgrading.

## 0.13.0

### Added

- **Judged fields can take several values.** `FieldDefinition.Multiple` marks a field whose value is a set; its values
  are settled in `SettledDocument.Sets` (`SettledDocument.Validate` checks that each field is kept where its kind
  belongs), with `Settlement.Set` and `Settlement.Restore(values)`. The settled field memory remembers each value on
  its own — one document's values weigh alike, as one settlement — and suggests each on its own, so several
  candidates can each be trusted; the values already chosen are evidence for the rest (`FieldMemory.Rank`'s
  `knownSets`) and are not offered again. A form session keeps the field open once settled, `FormResolver.SuggestAsync`
  has an overload that takes the chosen values, each value of a set is a key and an evidence line of its own for other
  fields, and `ThresholdSelection.SelectKeyThreshold` replays the values as picked one after another. The similar
  document layer does not suggest such a field, so it takes no `MemoryThreshold`. Nothing changes for forms without
  one.

- **A field's domain can come from another field of the same document.** `FieldDefinition.CandidatesFrom` names a
  field whose value — or, for a field that takes several, whose chosen values — bounds this field's candidates once it
  has any: the main topic among a document's topics, a subcategory within its category. Every layer and every threshold
  replay leaves values outside it out, as they do for `Candidates`, which still applies; a value settled outside it is
  still remembered. `FieldDefinition.Admits` has an overload that takes the document's values.

- **Suggestions while a person types into a judged field.** `FormResolver.SuggestAsync` and `FormSession.SuggestAsync`
  take, for each field being typed into, the text typed so far: the field stays open and its suggestion offers only
  values that begin with the text (ignoring case). Only the key layer answers then, held to
  `FieldDefinition.TypedKeyThresholds` for that many characters (`KeyThresholdFor`); a similar document's value that
  fits is a guess, and no model is asked. `ThresholdSelection.SelectTypedKeyThresholds` chooses them after
  `KeyThreshold`, each length on the documents a person types that far — those the thresholds before did not answer
  rightly. `FieldMemory.Rank` takes the typed text too. Not for a field that takes several values, which typed text
  only narrows.

### Changed

- **Threshold replays ask with the sets that had arrived before the field.** A single-valued field whose evidence
  includes a field with several values is now replayed with that field's values as keys, as it is suggested.

## 0.12.0

### Added

- **`ThresholdSelection.SelectDependsOn` chooses the fields a judged field should rest on.** It replays the field's key
  layer with each evidence field alone, adds them in order of how often they answer at the target precision while each
  addition answers more often, and returns that set (`DependsOnChoice.DependsOn`) only if it answers more often than
  every evidence field together, with every replay it tried. Where a few fields decide a field with many values and
  the rest only blur it, resting on those few answers more often at the same precision.

### Changed

- **README: `DependsOn` for fields with many values, and filtering candidates by typed text.** Two measured
  recommendations: name the fields that decide a field with many values in `DependsOn` (chosen with
  `SelectDependsOn`), and, while a person types into a judged field, filter a longer candidate list by the typed text
  instead of dropping it.
- **A form's judged fields no longer embed the same evidence text once each.** `EmbeddingMemory` keeps the vectors of
  the texts it embedded recently, so the judged fields of one document — remembered or suggested by the same evidence,
  one task each — cost one embedding call instead of one per field. `FormResolver.RebuildAsync` has the document memory
  embed its documents' evidence texts ahead of the writes, in batches of up to 64 distinct texts, instead of one call
  per field per document. Memory holds the same entries in the same order as before (an embedding model whose vectors
  vary in the last digits with the batch they are computed in may differ there, as `EmbeddingMemory.RebuildAsync`
  already could). Where judged fields are evidence for each other, each field still reads a different text; those texts
  are now embedded in batches too.

### Dependencies

- `IronHive.Providers.OpenAI.Compatible` 0.45.1 → 0.50.0 (`Gil.IronHive`). Nothing `Gil.IronHive` uses changed shape;
  its connect timeout and retries stay Gil's own settings.

## 0.11.0

### Breaking

- **Threshold replays ask about each judged field with the values that had arrived before it, not the whole settled
  document.** Where judged fields are evidence for each other (a field without `DependsOn`), a field's last suggestion
  is made before the fields settled after it have values, yet the replays (`SelectKeyThreshold`, `SelectAsync`,
  `SelectLayersAsync`) looked it up with all of them — so the precision a chosen threshold promised was measured on more
  evidence than suggestions get, and suggestions fell short of it. `SettledDocument` gains `Arrival`, the order the
  document's values arrived in, and `FormSession.Snapshot()` fills it in; a replay looks each field up with the values
  ahead of it in that order and still remembers each document with all its values, as the form resolver does. Without
  an order, observed values count as there from the start and judged ones as settled in the form's order. What to do:
  save `Arrival` with a snapshot and pass it back when choosing thresholds, or leave it out where the form order is the
  order people fill in; then choose thresholds again. A form whose judged fields depend only on observed fields replays
  as before, unless an observed value changed after a field was settled. Code that constructs `SettledDocument` by position compiles unchanged; recompile against this version.

### Changed

- **README: a chosen threshold holds only for the Gil version that chose it.** A release may change how a layer scores
  or what the replay asks, as 0.9.0, 0.10.0 and this release did. Store the Gil version with a threshold and choose
  again when it changes.

## 0.10.0

### Breaking

- **`SqliteTelemetryStore` moved to its own package, `Gil.Sqlite`; `Gil` no longer depends on SQLite.** A consumer that
  kept no telemetry — only memories and the form resolver, say — still received `Microsoft.Data.Sqlite` and its native
  library, which a self-contained or Native AOT publish then shipped beside the executable unused. The store keeps its
  namespace (`Gil.Telemetry`), its constructor and its file layout, so existing files open as before. What to do: if you
  use `SqliteTelemetryStore`, add `dotnet add package Gil.Sqlite`. If you excluded SQLite's native assets yourself to keep
  them out of a publish (a direct `SQLitePCLRaw` reference with `ExcludeAssets`), remove that reference.

### Changed

- **A field's `Candidates` bound every layer, not only the model.** A judged field with a closed list could still be
  suggested a value outside it — by the key layer, by a similar document or as a guess — whenever such a value had been
  settled, for instance one since dropped from the list; it could even come first and be trusted. Now no layer offers a
  value outside the list. The key layer ranks only the values in it, so a threshold is compared with the best of those;
  the similar-document layer offers nothing when the most similar document's value is outside it (rather than a less
  similar document's), and `SimilarDocuments` leaves such documents out; a model's values outside it are dropped.
  Threshold replays (`SelectKeyThreshold`, `SelectAsync`, `SelectLayersAsync`) apply the same rule, so what they promise
  matches what is suggested. Such settlements are still remembered, and are suggested again if the list takes the value
  back. `FieldDefinition.Admits` tells whether a value lies in the list. What to do: nothing for a field without
  `Candidates`, or one whose settled values all lie in its list — suggestions and chosen thresholds are unchanged. If
  settled values fall outside a field's list, choose its thresholds again.
- **A `FieldMemory` lookup costs as much as a key has values, however long its history.** Every settlement used to send
  its key's weights back to be taken afresh, sorting all of that key's settlements on the next lookup, and a suggestion
  for an unsaved document scanned them all to see whether it was among them. A key present on nearly every document
  therefore made each suggestion slower as settlements accumulated. Now a settlement later than every earlier one under
  the key carries the weights along, and only one that arrives out of order or is taken away has them taken afresh —
  through the same fold, so either way reaches the same weights to the bit. Suggestions and chosen thresholds are
  unchanged: replaying public streams of settled documents gave the same suggestions and answers, with scores and
  thresholds equal to within 1e-12. Replaying about 95,000 log lines whose host and process recur on nearly every line
  took 11 seconds, where the first 22,000 had taken over nine minutes, and the time per line no longer grows.

### Fixed

- **Threshold selection no longer lets a document settled again be its own evidence.** `SelectKeyThreshold`,
  `SelectAsync` and `SelectLayersAsync` replay every settlement they are given; a document given twice under the same
  id — saved, then saved again — was asked about with its earlier version still remembered, so it answered itself and
  the replay promised more than suggestions deliver, since a suggestion for a saved document passes over its saved
  version. The replay now drops a document's earlier version before asking about the next. Results are unchanged when
  every id appears once.

## 0.9.0

### Breaking

- **The key layer ranks and trusts values by one score: the sum of their strengths under the document's keys.** A
  value's strength under a key is still its weighted count there over the key's weighted total plus one, so a key settled
  once adds at most 0.5. Values were ranked by the sum of their plain shares, but trusted by the strength of their
  strongest key alone, so the value most likely right was not always the one the threshold looked at, and agreement
  between keys counted for nothing. Now the same score ranks the values, is compared with `KeyThreshold`, is what
  `SelectKeyThreshold` and `SelectLayersAsync` replay, and is the keyed candidates' `FieldCandidate.Score`; `Evidence`
  names the key the value is strongest under. Replaying a public stream of settled documents with thresholds chosen for a
  precision of 0.8, the key layer answered about three times as often (641 answers against 215) at the same precision
  (0.81 against 0.77), and the first candidate was right slightly more often. **What to do**: a `KeyThreshold` chosen
  before this release is on the old scale — the new score reaches up to the number of keys — so choose it again with
  `SelectKeyThreshold` (or `SelectLayersAsync`) right after upgrading; until then it is too low and the key layer
  answers too readily. A threshold set by hand keeps its meaning for a document with a single key.

### Changed

- **A `LexicalMemory` lookup reads only the n-grams a row shares with the request.** Every n-gram lists the rows that
  hold it with their weights, side by side, and a lookup adds those up instead of walking every row's n-grams against
  the request's. Results are unchanged to the bit — the products are added in the same order — and a lookup over a large
  memory is about ten times faster: replaying two public streams of settled documents, suggesting six judged fields
  against 50,000 remembered documents took 60 and 35 ms at the median instead of 590 and 343 ms. The lists hold a copy
  of each row's weights, so what a row holds per n-gram takes about three quarters more space, and they are rebuilt
  whenever the weights are taken afresh — remembering a large history takes up to twice as long.

## 0.8.0

### Breaking

- **A form field without `MemoryThreshold` still looks for similar documents.** Null used to skip the similar document
  layer; it now means no promise from it. With a document memory, the nearest document is looked up, reported in
  `SimilarDocuments` and offered as a guess, so a field whose threshold could not be chosen keeps its evidence instead
  of losing it. `Recall.Threshold` is `double?`, null for a lookup made without a threshold, and a stored trace then has
  no `threshold` in its `recall`. Every judged field is now remembered and looked up in the document memory, which costs
  what it costs for a field with a threshold — an embedding per field and document with an embedding memory. **What to
  do**: nothing to keep the first guess you had, since the new one comes after the most frequent value; to keep a field
  away from the document memory, resolve it with a resolver that has none.
- **Guesses come in a new order**: values under a key below `KeyThreshold`, then the field's most frequent value, then
  the nearest document below `MemoryThreshold` — which used to come first — then the field's other values. Replaying
  two public streams of settled documents, the nearest document below its threshold was right less often than a weaker
  key or the most frequent value, and about twice as often as the next most frequent one (first guess right 0.28 and
  0.27 of the time in the new order, against 0.26 and 0.22 with the nearest document first).
  **What to do**: nothing, unless you relied on a guess's position; answers (`Trusted` candidates) are unchanged.

### Fixed

- **Every package carries the license text.** The packages declared Apache-2.0 by expression only, so a consumer
  collecting third-party notices from the packages found the name and no text; `LICENSE` now ships next to
  `README.md` in each package.

## 0.7.0

### Breaking

- **`ThresholdSelection` returns what the replay found even when no threshold meets the target.** `SelectAsync` and
  `SelectKeyThreshold` return a `ThresholdReplay` instead of a nullable `ThresholdChoice`, and `LayerThresholds.Key`
  and `LayerThresholds.Memory` are `ThresholdReplay`s. A field that fell short used to come back as `null`, losing how
  close it came; `ThresholdReplay.MostPrecise` now gives the best precision the layer reached on at least
  `minimumAnswered` answers, with `Lookups` and `Candidates` counting the lookups made and those that found anything.
  **What to do**: read the threshold from `Chosen` — `layers.Key.Chosen?.Threshold`,
  `(await ThresholdSelection.SelectAsync(…)).Chosen` — where you read the choice itself before.

### Changed

- **Forgetting a `LexicalMemory` row takes constant time** instead of renumbering every row after it — a case replaced
  by a newer document, as the form resolver and `ThresholdSelection` do on every settlement, no longer costs a pass over
  memory. Results are unchanged: ties still go to the answer remembered first.
- **README guidance on guesses and on the similar document layer.** A guess should not be filled in or marked as the
  suggestion, but listing guesses as unmarked choices is fine — replaying settled documents, it saved about a third of
  the typing against one percent for answers alone. A form whose judged fields rest only on observed fields gets most
  answers from the similar document layer, which can fall short of a high target where documents come in batches that
  share their observed values.
- **README: what `targetPrecision` promises.** It is met on the documents the threshold is chosen on. Where settled
  documents come in batches alike in their observed values, the documents that follow can fall short of it — replaying
  two public streams, 0.63–0.74 for a target of 0.8.

## 0.6.0

### Breaking

- **`FormResolver`'s constructor gained an optional `similarDocumentCount` parameter** (after `timeProvider`). Source
  compiles unchanged; a binary built against 0.5.x that constructs a `FormResolver` must be rebuilt.

### Added

- **`ThresholdSelection.SelectLayersAsync(fieldMemory, memory, form, field, documents, targetPrecision, minimumAnswered)`**
  chooses a field's `KeyThreshold` and `MemoryThreshold` together, in the order the form resolver consults the layers,
  and returns both as `LayerThresholds(Key, Memory)`. A similar document answers only where no key did, and those
  lookups are harder than the rest: a `MemoryThreshold` chosen on every lookup by `SelectAsync` promised more precision
  than the similar document layer then delivered on the documents that followed. Replaying a public stream of about
  10,000 settled documents, answers from a similar document were right 66–75% of the time under a threshold chosen for
  80%. `SelectAsync` on its own remains right for a field without a `KeyThreshold`.
- **`FieldSuggestion.SimilarDocuments`** — the most similar settled documents behind a suggestion's candidate from a
  similar document (document id, similarity, settled value), when the `FormResolver` is created with
  `similarDocumentCount`. They come from the lookup that made the candidate.
- **`IMemory.NearestAsync(task, state, count, traceId)`** ranks the nearest few remembered requests in one lookup.
  `LexicalMemory`, `EmbeddingMemory` and `CircuitBreakingMemory` implement it; the default, for other memories, returns
  only the nearest.

### Fixed

- **Editing a saved document no longer loses the similar document layer.** The saved version of the document was its
  own nearest document, and the lookup counted it as a miss, so the layer suggested nothing even when other documents
  were close. It is now passed over for the next most similar document, as the field memory already left the document's
  own settlements out.

## 0.5.2

### Fixed

- **`ThresholdSelection` no longer admits a band of answers whose own precision falls short of the target.** It
  chose the lowest score at which the answers above it, taken together, met the target — so many good answers high up
  could carry a band of weak ones below them. A form whose settled documents repeat one another was hit hardest: the
  repeats answered each other correctly by their own keys, and a key backing its value about one time in four then
  cleared `KeyThreshold` and answered. Precision is now fitted as a non-decreasing function of the score (isotonic
  regression), and the threshold is the lowest score down to which every band meets the target. Both
  `SelectAsync` and `SelectKeyThreshold` choose this way; thresholds may come out higher, answering less but as often
  right as asked. Choose thresholds again after upgrading.

## 0.5.1

### Changed

- **An open document gives a field model its evidence in the order the values arrived.** `IFieldModel.SuggestAsync`
  receives, from a `FormSession`, the supporting values oldest first, a changed value moving to the end — so the evidence
  only grows at its end as the document fills, and a model that sends it to an inference server keeps the cached prefix
  of its previous request. Memory lookups keep the form's order (they match content, whatever order it was entered in),
  and a stateless `FormResolver.SuggestAsync` has no history, so its model still sees the form's order.

### Fixed

- **`Gil.IronHive`: a completion no longer fails when an alternative's log-probability is `null`.** llama.cpp sends a
  zero-probability alternative's logprob as `null` (JSON has no −∞); the OpenAI-compatible provider threw on it and the
  whole completion was lost. It now reads as zero probability. Requires `IronHive.Providers.OpenAI.Compatible` 0.45.1.

## 0.5.0

### Breaking

- **Values under a key answer only above `FieldDefinition.KeyThreshold`.** Before, a value settled alongside any known
  value led a field's suggestion, so a key that rarely decides the field — a choice among a handful of values that every
  document has — outranked a similar document, and the field's suggestions were mostly wrong. A key's strength is its
  weighted count for the value over the key's weighted total plus one: purity, discounted for few settlements. Without
  a `KeyThreshold` those values are now guesses. What to do: choose one with `ThresholdSelection.SelectKeyThreshold`,
  like `MemoryThreshold`; a null choice means the keys should not answer the field. Where neither a document memory
  nor a model is configured, candidates keep their order and only `Trusted`/`Answered` change.
- **`FieldCandidate` has a fifth parameter, `Trusted`** (default `true`). Code that deconstructs it positionally needs
  the extra element; `IFieldModel` implementations can keep constructing it with four.
- **A suggestion whose first candidate is a guess is traced as an abstention** (`Mode = "abstain"`, no output). Before,
  it was traced with the guessing layer's mode and value.

### Added

- **`FieldSuggestion.Answered`** — whether the first candidate is an answer rather than a guess.
- **Guesses after the answers**: the nearest similar document below `MemoryThreshold`, then values under a key below
  `KeyThreshold`, then the field's most frequent values. The nearest document was dropped before.
- **`ThresholdSelection.SelectKeyThreshold(fieldMemory, form, field, documents, targetPrecision, minimumAnswered)`**
  replays settled documents in the order they were settled and returns the lowest key strength that meets the target.
- **`FormResolver.SuggestAsync(form, documentId, values)`** suggests every open judged field from the values given and
  writes nothing to memory — for applications where saving is settling. The saved version of the same document is not
  evidence for itself. `FieldMemory.Rank` takes the document to leave out as `excluding`.

### Fixed

- The telemetry store no longer throws on a zero-probability alternative. A `TokenLogprob` whose `Logprob` is
  `-Infinity` is written as `"logprob": null` (JSON has no `-Infinity`; llama.cpp sends the same form) and read back
  as `double.NegativeInfinity`.

### Changed

- `Gil.IronHive` builds on IronHive 0.45.0.

## 0.4.0

### Added

- **`LexicalMemory.Nearest(task, state, count)`** lists the most similar remembered requests, most similar first
  (ties keep the order they were remembered in); the first is the match `LookupAsync` returns. Use it to offer several
  remembered answers as candidates rather than only the nearest one.
- **Form definitions** (`FormDefinition`, `FieldDefinition`, `FieldRole`, `FieldPolicy`) describe a form whose judged
  fields are suggested and settled one by one, with `SettledDocument`, `FieldCandidate`, `FieldSource` and
  `Settlement` for the values around them. A field can be kept out of every other field's evidence
  (`UseAsEvidence = false`) — for fields that identify a person, so that "this person, therefore this outcome" never
  hardens into memory. Declarations that contradict each other are refused when the form is defined.
- **`FieldMemory`** suggests a judged field's value from how often each value was settled alongside the values the
  document's other fields have, with no model and no call. A document contributes as a whole and putting it again
  replaces its contribution, so settling field by field and rebuilding from saved documents reach the same state in
  any order. Recent settlements weigh more (`recencyDecay`, 0.95 by default: a settlement counts 0.95 to the power of
  the number of later settlements under the same key), so a correction overtakes an older practice without first
  outnumbering it; ties go to the latest settlement. Pass `recencyDecay: 1` to count every settlement alike.
- **`SettledDocument.SettledAt`** — when a document was last settled. Where documents disagree the later settlement
  wins, so memory is the same whatever order documents arrive in, including a partial rebuild after live settling.
  Documents whose evidence for a field reads the same are one case, and a similar-document lookup answers with the
  case's latest settlement. In a session only accepting or correcting a field moves the time on; pass a reopened
  document's saved time to `Open` so that restoring it does not make its values new. `FormResolver` takes a
  `TimeProvider`.
- **`FormResolver` and `FormSession`** suggest a form's judged fields one document at a time. Open a document once,
  then pass values as they arrive: `ObserveAsync` for observed fields, `SettleAsync` with `Settlement.Accept`,
  `Correct`, `Reject`, `Revert` or `Restore` for judged ones. Each event returns fresh suggestions for the open fields
  whose evidence changed. A field is suggested from values settled alongside the known ones, then from a similar
  settled document (an optional `IMemory`, per field `MemoryThreshold`), then from its overall frequency; with none,
  a person decides. `Snapshot()` is the document to save and `RebuildAsync` puts saved documents back into memory;
  both replace a document's earlier contribution, so they can be combined freely. One trace per suggestion, under
  the task `form/field`.
- **`IFieldModel`** lets `FormResolver` ask a model for a field that neither memory had evidence for (never
  overriding one that did); its candidate goes ahead of the field's overall frequency and it alone reports a
  confidence. **`ResolverFieldModel`** puts the resolver behind it, one task per field. With the form resolver's sink
  and tasks named `form/field`, the resolution closes the suggestion's trace itself, so feedback reaches the tree.
- **`ThresholdSelection.SelectAsync`** chooses a field's `MemoryThreshold` by replaying settled documents in the order
  they were settled into an empty memory, one document per case as the form resolver keeps it: the lowest threshold whose answers reach a target precision, on at least a given number
  of answers, with its answer rate — or null when none does. The right threshold moves as memory grows, so choose it
  again as it grows rather than fixing it once.

## 0.3.0

### Added

- **`LexicalMemory`**, memory that needs no model: nearest neighbour over character n-gram TF-IDF (2- and
  3-grams by default), so it works in any script without a tokenizer and a lookup costs no call. Fill it from the
  feedback history with `MemoryReplay`. Its similarities are on their own scale, so choose the task's
  `MemoryThreshold` for it rather than reusing an embedding memory's. Weights are taken afresh as the memory grows by
  a tenth, so a similarity changes in steps rather than with every answer remembered.
- **`new Resolver(memory, sink)`**, a resolver without models: memory answers what it can and every other request
  abstains (`abstain`, empty path, null confidence, the miss in `Recall`) without a call. The task's tree,
  thresholds, contract and language are not used — a bare root will do. Memory learns from feedback only through a
  sink, as before.

## 0.2.0

### Breaking

- **The IronHive models are a package of their own, `Gil.IronHive`.** `IronHiveChatModel`, `IronHiveEmbeddingModel`
  and `OpenAICompatibleOptions` move there (namespace unchanged, `Gil.Llm`), and `Gil` no longer depends on IronHive.
  Add `dotnet add package Gil.IronHive`. A consumer that implements `IChatModel` and `IEmbeddingModel` itself needs
  only `Gil`.
- **Every task declares its prompt language.** `TaskDefinition` takes a fifth argument, `PromptLanguage`, with no
  default. Pass `PromptLanguage.English` or `PromptLanguage.Korean` — the wording 0.1.0 used, except that an answer
  without a description no longer gets an empty example in the tree contract — or change a piece of either with
  `with { ... }`:
  ```csharp
  new TaskDefinition("support", new TreeAnswerContract(tree), tree, policy, PromptLanguage.English)
  ```
- **Wording moved into `PromptLanguage`.** `JudgePromptTemplate`, `SingleTokenJudgeOptions.Prompt`,
  `FallbackPromptTemplate` and `FallbackGenerator`'s template argument are gone; set the same text on the language
  instead (`JudgeSystem`, `JudgeQuestion`, `NoneOfThese`, `FallbackSystem`, `FallbackPath`, `FallbackExamples`,
  `FallbackRetry`, …).
- **Contracts take the language.** `IOutputContract.Instruction()` and `Validate(text)` become
  `Instruction(PromptLanguage)` and `Validate(text, PromptLanguage)`. `TreeAnswerContract` no longer takes `None` or
  `Escape` (they are the language's `NoneOfThese` and `OutOfCategory`), and `TextContract` no longer takes `Language`
  (answers are in the task's language).
- **Narrowing returns a contract.** `IScopableContract.Scoped` returns `IOutputContract?`; `ScopedContract` is gone.
  A narrowed answer that escapes is `language.OutOfCategory`.
- **Components take the language per call**: just before `traceId` in `IJudge.JudgeAsync`,
  `GreedyTraverser.TraverseAsync`, `FallbackGenerator.GenerateAsync` and `SlotFiller.FillAsync`, and after `contract`
  in `Differentiation.Anchored`. Code that only uses `Resolver` needs no change beyond the task's language.
- **`OpenAICompatibleEmbeddingModel` is replaced by `IronHiveEmbeddingModel`**, the embedding counterpart of
  `IronHiveChatModel`: `IronHiveEmbeddingModel.OpenAICompatible(options)` takes the same `OpenAICompatibleOptions`,
  and `new IronHiveEmbeddingModel(generator, model, extraBody)` takes any IronHive `IEmbeddingGenerator`. Embedding
  requests follow the chat rule for `ExtraBody`: its fields are merged over the ones Gil sets (objects merge, other
  values replace), where 0.1.0 let `model` and `input` win.
- **`IMemory` separates the key from the trace**: `RememberAsync(task, key, state, answer, traceId)` and
  `Forget(task, key)`. `key` is the request the answer belongs to (what a lookup reports as the source); `traceId` is
  what the call's cost is recorded against. The resolver passes the request's trace id for both. An implementation
  that took `traceId` as the key should now use `key`.
- **Records gain positional members**, so constructing or deconstructing them by position changes:
  `Resolution` gains `Failure` (8th), and `TaskStats` gains `Misroutes` (9th). Both default to null.
- **`SqliteTelemetryStore.Stats` gains an optional `tree`** — source compatible, but code compiled against 0.1.0 must
  be rebuilt.
- **`DifferentiationKind` gains `Unserved`**: a `switch` over it needs the new case.
- **`PromotionReview` gains `Applied`**, the proposals the tree actually took; record those
  (`RecordPromotionRound`), not every proposal.

### Added

- `PromptLanguage` with `Korean` and `English` built in.
- `Gil.IronHive` package (see Breaking) and `EmbeddingGeneratorModel` in `Gil`: memory embeds through any
  Microsoft.Extensions.AI `IEmbeddingGenerator` (`Gil` now references `Microsoft.Extensions.AI.Abstractions`, which
  has no dependencies on .NET 10).
- Traces and metrics through `ActivitySource` and `Meter` named `GilDiagnostics.Name`: `gil.resolve` and `gil.call`
  spans, and `gil.resolutions`, `gil.resolution.energy`, `gil.resolution.duration` and `gil.call.energy`.
- `MemoryReplay` (`From`, `ApplyAsync`) and `ConfirmedAnswer`: the confirmed answers and overturned keys in a feedback
  history, applied to any `IMemory` — rebuild a memory store of your own from the log.
- Promotion rounds are recorded and read back: `IPromotionLog` (`RecordPromotionRound`, `PromotionRound`,
  `PromotionHistory`).
- Review decisions: `IReviewLog` (`RecordReview`, `Reviews`) with `ReviewKind`, `ReviewDecision` and `ReviewRecord`.
  `SqliteTelemetryStore` implements both logs.
- `Resolution.Failure`, `TraceOutcome.Failure` and a `failure` column on traces: why a request ended without output.
- `Stats(..., tree:)` counts misroutes (`TaskStats.Misroutes`, `MisrouteStats`): confirmed answers outside the
  category a request was sent to.
- `Differentiation.UnservedAsync` and `DifferentiationKind.Unserved`: similar requests the task keeps answering
  "none of these" to.
- README sections on when to use Gil, the tree file, memory rebuilds after a restart, bringing your own memory store,
  corrections in promotion, and traces and metrics.

### Changed

- The README and package descriptions lead with the parts whose effect was measured — confirmed-answer memory,
  one-token judgments and narrowed generation, with every call priced — and describe habit promotion as the
  experimental, off-by-default feature it already was. No behaviour change.
- IronHive 0.41.0: embeddings go through its OpenAI-compatible embedding generator, which reports the server's usage
  and model. The default transport is IronHive's connection-racing handler, so a `localhost` server that listens on
  IPv4 only is reached without first waiting out the IPv6 attempt.
- Trees are written without fields equal to their defaults, so a reviewed file diffs cleanly against a hand-written
  one. Files written by 0.1.0 still load.
- `EnergyModel.Fit` returns the best non-negative fit; a fit that was already non-negative is unchanged.
- A path step prints its probabilities.
- The tree contract gives no empty example for an answer without a description, in either language; in English it
  asks for a line of the list.

### Fixed

- A rate's 95% interval is exactly 0 or 1 at 0% or 100%, not off by rounding.
- Disposing a telemetry store no longer clears the connections of other stores in the same process.
- Opening a store written by an older version adds the columns it lacks.

## 0.1.0

First release: memory, the tree of one-token judgments, narrowed and full fallback, slot filling, the telemetry store
with priced calls, habit statistics, shadows, and promotion, deactivation and differentiation proposals.
