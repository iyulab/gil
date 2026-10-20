# Changelog — gilfit

The Rust crate `gilfit` (crates.io) and the Python package `gilfit` (PyPI) share this version line. Versions follow
`0.x`: a minor release may change the public API, and each such change is listed under **Breaking** with what to do.
The .NET library on NuGet (`Gil`) keeps its own line in the repository's root `CHANGELOG.md`.

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
