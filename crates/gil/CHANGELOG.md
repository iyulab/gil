# Changelog — Gil (Rust crate and Python package `iyulab-gil`)

The Rust crate (crates.io) and the Python package (PyPI), both published as `iyulab-gil`, share this version line.
Versions follow `0.x`: a minor release may change the public API, and each such change is listed under **Breaking**
with what to do. The .NET library on NuGet (`Gil`) keeps its own line in the repository's root `CHANGELOG.md`.

## Unreleased

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
