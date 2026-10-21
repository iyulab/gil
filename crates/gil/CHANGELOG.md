# Changelog — Gil (Rust crate and Python package `iyulab-gil`)

The Rust crate (crates.io) and the Python package (PyPI), both published as `iyulab-gil`, share this version line.
Versions follow `0.x`: a minor release may change the public API, and each such change is listed under **Breaking**
with what to do. The .NET library on NuGet (`Gil`) keeps its own line in the repository's root `CHANGELOG.md`.

## Unreleased

### Fixed

- **Python: a record that keeps an attribute where its kind does not belong is a `ValueError` and leaves the decider
  usable.** `Decider(schema, records)` and `Decider.put` handed it to the core, which took it for a broken contract
  partway through the change, and the decider could no longer be used; they now check the records first.
- **A lookup of an attribute the schema does not have is refused whatever memory holds.** `Lookup::new` took any
  name, and the key memory's reads (`rank`, `first`, `typed`, `keyed_values`, `key_totals`, `scored`, `catches`)
  answered an unknown name with nothing while the schema's memory was empty and panicked once it held records. It now
  panics at once, naming the attribute.
- **A record or lookup that keeps an attribute where its kind does not belong is refused instead of half-read.** A
  single value for an attribute that takes several values, or a set for one that takes one value, was skipped without a
  word by `KeyMemory::put` (and so `Decider::rebuild` and every replay) and by `Decider::answer` / `answer_with` /
  `answer_one` — the settlement was lost from memory, or the evidence from the answer. They now panic with the attribute
  named, as `OpenRecord::reopen` already did; `Schema::check_placement` tells beforehand.
- **The key layer no longer slows down with the square of an attribute's distinct values.** Gathering each value's score
  under the known keys, and filling the candidates from the attribute's overall values, each searched a growing list
  for every value; an attribute with thousands of values (a product code, say) spent most of an answer there. Answers
  and threshold selections are unchanged — the same values, scores and order — and on two public replays of 10,000
  decisions each, a replay of answers took 38% and 67% of the time it did.
- **The similar record lookup is quicker without changing its answers**: the rows' norms sit beside the rows, and the
  sorts that break ties (the lookup's, the key layer's, replay orders) read their tie-breakers only on a tie. On 10,000
  lookups over 20,000 remembered texts, 22.4 s became 17.9 s.
- **Remembering a record again or forgetting one no longer walks the lists of every n-gram it holds.** A decider
  forgets and remembers a record's text each time the record is settled, and each time the similar record memory took
  the record out of the list of each of its n-grams by searching it — for an n-gram every record shares (an attribute's
  name), the whole memory — and the lists lost their order, so later lookups wrote their sums all over memory. Rows now
  keep a slot of their own: a row replaced or forgotten leaves its entries behind until they pass an eighth of the live
  ones, and the lists are rebuilt in order. Answers are unchanged to the bit. Over 20,000 remembered texts, remembering
  each again took 1.2 s instead of 14.6 s, and 10,000 lookups after that took 19.5 s instead of 29.8 s (17.1 s instead
  of 22.8 s on a memory remembered once).
- **The key layer folds each key's settlements as they arrive instead of again on every look.** Every score sorted all
  the settlements under each of the record's keys and weighed them from the earliest, so a replay that chooses
  thresholds — a look per record for every record before it — spent most of its time sorting. Each key now keeps its
  weights folded as settlements arrive in order and its shares until the next change; one that arrives out of order or
  is taken away has the fold run again once, on the next look. Scores are unchanged to the bit. On two public replays
  of 10,000 decisions each, choosing the thresholds took 74% and 80% of the time it did.

### Added

- **`Settlement::new(kind, value, values, probability)`: a settlement of a kind known only at run time, from the parts it
  needs.** It refuses, with a message, a part the kind needs and lacks and a check's probability outside (0, 1] — the
  rules a binding used to repeat. The Python package's `OpenRecord.settle` now builds its settlements with it (no change
  in what it accepts).
- **`OpenRecord::check_observe` and `OpenRecord::check_settle`: what `observe` and `settle` would refuse, told before
  anything changes** — an attribute the schema does not have, one of the other role, a settlement of the other kind of
  attribute — as a `ContractViolation` with the message the call panics with. Both calls check through them, so the
  rule lives in one place. Also `Schema::find`, the attribute named, if there is one.
- **`Decider::remove(schema, record)`: takes a record out of both memories** — a record the host deleted, without
  rebuilding. The key memory is then as if it had never been put; the similar record memory never offers it again
  (its similarities follow the lexical memory's periodic refresh, so they can differ slightly from a rebuild's).
- **`Decider::answer_with`: `Decider::answer` with each lookup's decision record handed to a sink**, in the answers'
  order — for a host that keeps them to recount its thresholds later.
- **`OpenRecord::answer_with`: `OpenRecord::answer` with each lookup's decision record handed to a sink**, as
  `Decider::answer_with` does for a stateless answer.
- **`AskPlanner::attribute`: the decided attribute a planner plans for** — so that a host holding several planners can
  hand each answer to its own.
- **Python: `Decider.remove(schema, record_id)` and `Decider.answer_with_decision(...)`** — the same two in the Python
  package: a deleted record taken out without a rebuild, and an answer returned with its lookup's decision record.
- **Python: `OpenRecord.answer_with_decisions(decider)`** — an open record's answers, each with its lookup's decision
  record.
- **Python: the replay constants** `RECENT_RECORDS`, `POOLED_STANDARD_ERRORS`, `POOLED_RECENT_LOOKUPS` and
  `POOLED_RECENT_ANSWERS`, as the crate has them — how far back a choice's promise and the pooled rule look.

### Changed

- **Breaking: the answer sources `Source::SettledFieldMemory` and `Source::SimilarDocument` are `Source::KeyMemory` and
  `Source::SimilarRecord`**, in the crate's vocabulary — in Rust, in what `Source::name` and the `serde` feature write
  (`"KeyMemory"`, `"SimilarRecord"`), and in the strings Python answers carry. A host that stored answers or decision
  records replaces the two strings. `Model` is unchanged. The C ABI's values are unchanged; its constants are
  `GIL_SOURCE_KEY_MEMORY` and `GIL_SOURCE_SIMILAR_RECORD`.
- **Breaking: `replay::RECENT_DOCUMENTS` is `replay::RECENT_RECORDS`**, in the crate's vocabulary. Same value.
- **Python: a threshold selection refused by the core's contract raises `ValueError` with the core's own message** —
  an attribute that is not decided (`'a' is not a decided attribute`), a target outside (0, 1], an attribute with no
  coarse level. The binding no longer repeats those checks with wording of its own, and the key attributes' and
  concentrations' selections now raise `ValueError` for them too instead of a panic.
- **Python: a defect while a decider or record changes now makes them unusable instead of leaving them half-changed.**
  `Decider.put`, `OpenRecord.observe` and `OpenRecord.settle` check their contract first (still `ValueError`, after
  which both objects are as they were); if the core then fails partway, they raise `RuntimeError`, and every later call
  on the decider and the record raises `RuntimeError` too — build them again.
- **A schema whose coarse level has an empty class table or gives a value a blank class is refused**, as the
  reference implementation does. Such a schema used to be accepted; the Python `Schema` and the .NET native binding's
  schema refuse it too, since both check through the core.
- **Breaking: `AskPlanner` keeps the schema it was made for; `learn(decider, record)` no longer takes one.** A planner
  given another schema to learn with learned from that schema's settings while its cached class sizes stayed those of
  the first. Drop the schema argument: `planner.learn(&decider, &schema, &record)` becomes
  `planner.learn(&decider, &record)`; `AskPlanner::schema()` reads it back. The Python and .NET bindings' calls are
  unchanged.
- **Breaking: a `Schema` is checked when it is made.** Its fields were public and nothing in the crate called
  `Schema::check`, so a schema built in Rust that broke a rule (a key attribute it does not rest on, no decided
  attribute, a dependency on an attribute it does not have, …) was used as it was and failed later or behaved oddly.
  Now no invalid schema can exist:
  - `Schema { name, attributes }` becomes `Schema::new(name, attributes)?`, which returns the message of the first rule
    broken — the same messages `check` gave.
  - `schema.check()` is gone: a schema in hand has passed it.
  - The fields are private: read them with `schema.name()` and `schema.attributes()`. To change a schema, build the
    attributes again (`schema.attributes().to_vec()`, edit, `Schema::new(schema.name(), attributes)?`).
  - Deserializing (feature `serde`) refuses a schema that breaks a rule, with the same message; the JSON form is
    unchanged.
  - `Tuning::apply(&self, schema: &Schema) -> Result<Schema, String>` returns the tuned schema instead of changing one
    in place, and refuses a result that breaks a rule — a tuning chosen for another schema may name key attributes this
    one does not have.

  The Python and .NET bindings already checked every schema they read; a schema that breaks a rule is still refused
  (`ValueError`, `ArgumentException`), now with the message prefixed by `a schema does not read:` and followed by where
  in the JSON reading stopped. Applying a tuning that would break the schema still raises `ValueError` in Python, and
  now throws `ArgumentException` in .NET, where the tuned schema used to be handed out unchecked.

## 0.7.0

### Added

- **`ask_planner::AskPlanner`: which observed attribute to ask a person for first, so that a decided attribute gets
  answered.** It learns from settled records, in settlement order, what asking each attribute gained per character typed
  (`learn`), and advises an open record's missing attributes by that gain, marking those worth asking within a budget
  of the typing asking every time would take (`advise` → `AskAdvice`). Experimental, as in the .NET library, whose
  `AskPlanner` it reproduces to the bit (a new golden, `tests/fixtures/core/ask.jsonl`). Python: `gil.AskPlanner`
  (`learn(decider, record)` · `advise(answer, values)`).
- **Python: `Decider.put(schema, records)` and `Decider.answer(schema, id, attribute, values, sets=None)`.** `put` adds
  records settled later than those the memories hold — what learning in settlement order needs. `answer` is one open
  decided attribute's answer from the values given, writing nothing to memory (the core's `Decider::answer_one`); an
  `OpenRecord` puts its record into memory with each change.

## 0.6.0

### Added

- **`replay::ThresholdRule` and `replay::select_layers_by_rule` (`_with`): choose an attribute's two thresholds for a
  target on its answers together.** `ThresholdRule::EveryBand` is the rule of `select_layers`, unchanged.
  `ThresholdRule::Pooled` chooses the key and memory thresholds as the pair that answers the most of the attribute's
  latest `POOLED_RECENT_LOOKUPS` lookups while the precision of all those answers holds the target at the bottom of its
  Wilson interval at `POOLED_STANDARD_ERRORS` (1.645) standard errors, reaching back while the attribute's current
  thresholds answered fewer than `POOLED_RECENT_ANSWERS` of them. Python: `select_layers(..., rule="pooled")`. The
  same rule as the .NET library's `ThresholdRule.Pooled`, to the bit.

### Documentation

- How to choose `replay::Choice::target_precision` from what a wrong answer costs: answering pays only above
  `1 − 1/ρ` when a wrong value costs ρ entries (README, rustdoc).
- `replay::select_key_concentrations`: a concentration under 0.01 can mean an attribute written from the decided value.

## 0.5.0

### Breaking

- `SettlementKind` has two more variants (below): a `match` over it that names every kind needs arms for
  `AcceptOther` and `CorrectOther`.

### Added

- **A value a person settled over a suggestion that was not the decider's.** A host may show its own suggestion — its
  own model's, a rule's, a default — in place of an answer. `SettlementKind::AcceptOther` (kept) and
  `SettlementKind::CorrectOther` (changed), with `Settlement::accept_other` and `Settlement::correct_other`: memory
  learns the value and choosing thresholds counts it as it counts the others, but it grades no answer. Settling such a
  value as `Enter` would put it among the values settled with nothing in view, and as `Fill` would keep it out of
  memory. A value the host's model gave the decider through `offer_external` comes back in the answer, and taking that
  answer stays an `Accept`. `usage` counts the two on their own (`accepted_other`, `corrected_other`), outside the
  realized precision. Python reads both names in a record's `settlements`.
- **`serde` feature** (off by default): `Serialize` and `Deserialize` for `Schema`, `Attribute`, `CoarseLevel`,
  `AttributeRole`, `AttributePolicy`, `Record`, `Settlement`, `SettlementKind`, `Tuning`, `AttributeTuning`, `Stale`
  and `AttributeUsage` — a host keeps tunings and snapshots without a copy of their shape. The form is the one the
  Python bindings read: Rust field names, snake-case roles and policies, settlement kinds by name, settings left out
  taking `Attribute::new`'s defaults, unknown settings refused.
- `serde` also covers the replay results (`ThresholdChoice` · `ThresholdReplay` · `KeyState` · `LayerThresholds` ·
  `TypedKeyThresholds` · `KeyFieldsTrial` · `KeyFieldsChoice`), the decision records (`DecisionRecord` and its looks)
  and the answers (`Answer` · `Candidate` · `Evidence` — `{"key": {"attribute", "value"}}` or `{"record": {"id",
  "similarity"}}` · `ExternalCandidate` · `key_memory::Key`), with `Source` written as the reference names the layers
  (`SettledFieldMemory`, `SimilarDocument`, `Model`) — a host passes an answer on as it is.

- **Python: answering a record while its values arrive, tunings and usage.** `gil.Decider` (the memories, from
  settled records) and `gil.OpenRecord` (`observe` · `settle` by kind name · `answer` · `answer_partial` ·
  `offer_external` · `snapshot` · `reopen`), with answers as typed dictionaries (candidates with structured evidence,
  policy, `needs_external`); `gil.capture_tuning` · `gil.record_promise` · `gil.apply_tuning` · `gil.tuning_stale`; and
  `gil.usage`. A call the core refuses raises `ValueError` with the core's message.

### Changed

- Python: decision records also carry `typed` (on a typed replay) and `answered` (what a suggestion answered), as the
  core's records do; results are written through the core's `serde` form.
- Python: schemas and records are read through the core's `serde` form. An unknown or missing setting is now reported
  in serde's words (``unknown field `colour` `` · ``missing field `role` ``); what reads is unchanged — times as text
  or ticks, entries given as `None` left out.

## 0.4.0

### Breaking

- **The core speaks its own vocabulary, not a form's.** Gil decides attributes of records in many shapes — issues,
  log lines, patents as well as forms — so the public names say so. Renamed in Rust and Python; behaviour, scores and
  choices are unchanged (the goldens reproduce bit for bit). What to do — replace the names:

  | Up to 0.3.0 | Now |
  |---|---|
  | `gil::form` · `Form` · `Field` · `Role::{Observed, Judged}` | `gil::schema` · `Schema` · `Attribute` · `AttributeRole::{Observed, Decided}` |
  | `Form::fields` · `Form::field(name)` | `Schema::attributes` · `Schema::attribute(name)` |
  | `Document` | `Record` |
  | `gil::form_resolver` · `FormResolver` · `Suggestion` | `gil::decider` · `Decider` · `Answer` |
  | `FormResolver::suggest` · `suggest_one` · `suggest_one_with` | `Decider::answer` · `answer_one` · `answer_one_with` |
  | `gil::field_memory` · `FieldMemory` | `gil::key_memory` · `KeyMemory` |
  | `gil::threshold_selection` | `gil::replay` |
  | `Field::similar_document_votes` · `memory_similarity_floor` | `Attribute::similar_votes` · `memory_floor` (also on `CoarseLevel`) |
  | `Lookup::form` · `Lookup::field` · `DecisionRecord::field` · `SimilarLook::document` | `Lookup::schema` · `Lookup::attribute` · `DecisionRecord::attribute` · `SimilarLook::record` |
  | `FormResolver::new(field_memory, document_memory)` | `Decider::new(key_memory, record_memory)` |

  Python: `gil.Form` → `gil.Schema`, `gil.Documents` → `gil.Records`, `gil.documents(...)` → `gil.records(...)`,
  `check_form` → `check_schema`; every `field` argument is `attribute`; the flag that returns decision records is
  `decisions=` (was `records=`). A schema's JSON is `{"name": ..., "attributes": [{"name": ..., "role": "observed" |
  "decided", ...}]}` (was `fields` · `judged`), with `similar_votes` · `memory_floor` for the renamed settings. A
  decision record's `field` is `attribute`, and a similar look's `document` is `record`. Records' JSON lines are
  unchanged.
- **A candidate's evidence is structured.** `decider::Candidate::evidence` is `Option<Evidence>` —
  `Evidence::Key(Key { attribute, value })` for a key (the value as the key memory keeps it, normalized and cut to the
  key length) or `Evidence::Record { id, similarity }` for a similar record — in place of the string
  `"attribute: value"` or the record id. What to do: match on it; `to_string()` gives the old text. Likewise
  `key_memory::Candidate::evidence` is `Option<Key>` and `KeyMemory::keyed_values` returns `Key`s. Decision records keep
  the text (`KeyLook::key`).
- **Python rejects a schema the core would not take**: no decided attribute, a duplicate name, settings on an attribute
  they do not fit (see `Schema::check`). Such a schema used to read and fail later or behave oddly.

### Added

- **`open_record::OpenRecord`** — a record while its values arrive, as the .NET `FormSession` keeps one: `observe` an
  observed attribute, `settle` a decided one (with the time it happened — the core keeps no clock), and get fresh
  answers for the attributes whose evidence changed; `answer`, `answer_partial` (a value seen in part, such as typed
  text), `snapshot` (the record to save, with the order its values arrived in and how each was settled) and `reopen`
  (a saved record as its snapshot left it). Rejected attributes stay closed until reverted or settled; an attribute
  that takes several values stays open once settled.
- **`Attribute::candidates_from`** — another attribute whose value in the same record bounds this one's domain (for an
  attribute that takes several values, the values chosen in it): every layer, an external judgement and every replay
  offer only values inside it once it has a value; while it is empty the domain is not narrowed. `Attribute::admits_in`
  answers for a record; `admits` keeps answering for `candidates` alone. Python reads it as `"candidates_from"`; a
  schema whose attribute takes its candidates from itself or from an unknown attribute is rejected.
- **`Schema::check`** — the rules the .NET library applies when a form is made: unique names, at least one decided
  attribute, settings that fit their attribute, dependencies · key attributes · key concentrations · `candidates_from`
  that name other attributes. The Python binding applies it to every schema it reads.
- **`tuning::Tuning`** — the settings the replays chose for a schema as one plain record a host keeps between runs:
  per decided attribute the key attributes, key concentrations, key · typed key · memory thresholds, floor and coarse
  thresholds, with the promises the replays made (`Tuning::promise`), plus the crate version, a fingerprint of the
  settings the choice rests on (`tuning::fingerprint`) and how many records were settled. `Tuning::capture` takes it
  from a schema, `Tuning::apply` puts it back, and **`Tuning::stale`** says when to choose again: another crate version
  (`tuning::VERSION`), a changed setting the choice rests on (role, domain, dependencies, votes, coarse level …), or
  settled records grown or shrunk by a tenth since the choice.
- **`usage::usage`** — what the answers came to, counted without content, per attribute: decisions, answered and
  abstained, accepted · corrected · rejected · filled · checked (and how many checks agreed) · entered, the realized
  precision (`AttributeUsage::realized` — accepts right, corrections wrong, checks weighted by one ÷ their probability;
  fills and rejects left out) against the mean promise (`AttributeUsage::promise`). An answered decision settled as
  entered — a host reporting its own answers' acceptance as entry, which grades the answers on themselves — is counted
  apart (`entered_over_answer`) and should stay 0.
- **`schema::Settlement`** — `accept` · `correct` · `enter` · `fill` · `check(value, probability)` · `restore` · `set`
  · `restore_set` · `reject` · `revert`.
- **`schema::AttributePolicy`** (`Suggest` · `ConfirmRequired` · `Off`) on `Attribute::policy`: an attribute that is
  off is never answered; every `Answer` carries its attribute's policy, so a host fills on its own only what does not
  require confirmation. Python reads it as `"policy": "suggest" | "confirm_required" | "off"`.
- **External judgement without a callback.** Where neither memory has evidence and nothing is typed,
  `Answer::needs_external` carries the (attribute, value) pairs a model would read — in the order they arrived, or the
  schema's order without a history. The host asks its model, or does not, and hands the ranked values back with
  `Decider::offer_external` (or `OpenRecord::offer_external`): they join the candidates after the values a memory
  trusts and before the guesses (`Source::External`), values outside the attribute's domain or already chosen left
  out. `Schema::evidence_values` gives the same pairs; `Lookup::arrival` orders them.

## 0.3.0

### Breaking

- **Renamed.** The Rust crate and the Python package are published as `iyulab-gil` and imported as `gil`; they were
  `gilfit` up to 0.2.0, and `gilfit` 0.1.0 and 0.2.0 are yanked. The API is unchanged. What to do:
  - Rust: depend on `iyulab-gil = "0.3"` and write `use gil::…` in place of `use gilfit::…`.
  - Python: `pip install iyulab-gil` (uninstall `gilfit`) and `import gil` in place of `import gilfit`. The package is
    `gil`, the same name as another distribution on PyPI; do not install both into one environment.
- The repository is https://github.com/iyulab/gil again; the `gilfit` address redirects.

## 0.2.0

### Rust crate

- No changes; published with the Python package, which shares the version line.

### Python package

- **Added** `gilfit.select_key_fields`: chooses the fields that should key a judged field by replaying its key layer —
  each field alone, then greedy additions while the gain over the target precision grows — the same choice the core's
  `select_key_fields` and the .NET `ThresholdSelection.SelectKeyFields` make. Set its `key_fields` on the judged field
  before `select_layers`. Without it a field that happened to settle with one value for a while keys every later
  document that shares the value, and the key layer can answer below the promise its replay made. Results are typed
  (`KeyFieldsChoice`, `KeyFieldsTrial`, `TypedKeyThresholds`); `typed_longest` counts the answers while a person types,
  as in the core.
- **Added** `gilfit.select_key_concentrations` (how readily each key field's keys are believed after a few
  settlements — set as the field's `key_concentrations`) and `gilfit.select_coarse` (the coarse level's thresholds on
  the lookups the value layers leave, with decision records). With `select_key_fields` and `select_layers` the Python
  package now covers each step of choosing a field's settings that the core and the .NET library offer, except typed
  key thresholds on their own (`select_key_fields(typed_longest=…)` returns them with the key fields).
- Decision records carry `coarse` (`CoarseLook`): on a coarse replay, the class the keys back, the class the similar
  documents vote for and whether the value layers answered; `None` on other replays.

## 0.1.0

First release.

### Rust crate

- **Text** — normalization, keys and character n-grams as `docs/text-normalization.md` specifies; typed text and
  prefixes count Unicode scalar values.
- **Settled field memory** — values settled together, weighted toward the more recent, for fields that take one value
  or several; values a field must not miss hold the layers back where they were settled under the document's keys.
- **Lexical document memory** — the nearest settled documents by character n-gram TF-IDF cosine, looked up through an
  index of the n-grams each remembered text holds, vote for a value; no model.
- **Threshold choice by replay** — key, similar document, coarse level and typed key thresholds, chosen by replaying
  the settled documents in the order they were settled, each with the promise a replay of that many answers can make;
  key fields chosen by the gain over the target, and key concentrations.
- **Form resolver** — keeps both memories up to date as documents are settled and suggests each open judged field:
  trusted values first, then guesses, the coarse class when no layer answered, and suggestions while a person types.
- **Decision records** — replays and suggestions hand every lookup to a caller's sink (`select_layers_with`,
  `select_coarse_with`, `select_typed_key_thresholds_with`, `suggest_one_with`).
- The core does no I/O. It reproduces the .NET library's language-neutral goldens (`tests/fixtures/core`) to the bit.
  Minimum Rust version 1.90.
- Not carried over from the .NET library yet: a model layer and telemetry.

### Python package

- `gilfit.select_layers` replays settled documents and returns the key and similar document threshold replays with
  the decision records they rest on, as typed dictionaries (`LayerThresholds`, `DecisionRecord`); `Form` and documents
  are read from JSON (`Form.from_dict`, `Form.from_json`, `documents`, `read_records`).
- Typed (`py.typed`, stubs for the extension module). One wheel per platform for Python 3.11 and later (abi3).
