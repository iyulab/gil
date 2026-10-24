# Gil (Python — `iyulab-gil`)

Gil answers the decided attributes of a record from the records people have already settled, and chooses — by
replaying them in the order they were settled — how far each answer can be trusted and how often a trusted one will
be right.

This package binds its Rust decision core: replay settled records to choose how far each layer's answers for a
schema attribute can be trusted, and get back the decision records the choice rests on.

```sh
pip install iyulab-gil
```

```python
import gil

schema = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "product", "role": "observed"},
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "decided"},
        ],
    }
)
records = gil.read_records(
    "settled.jsonl"
)  # one JSON object per line, in settlement order
thresholds, decisions = gil.select_layers(
    schema, "team", records, target_precision=0.8
)
print(thresholds["key"]["chosen"], len(decisions))
```

A line holds a record's `id`, `settled_at`, `values` and, for each decided attribute, how it was settled
(`settlements`) — the form [docs/settled-records.md](https://github.com/iyulab/gil/blob/main/docs/settled-records.md) specifies, which the other bindings read too.
Records kept in it before Gil is in use are what the first thresholds are chosen from.

Before choosing thresholds, narrow the attributes that key the decided attribute. An attribute that happened to settle
with one value for a while would otherwise key every later record that shares it:

```python
choice = gil.select_key_fields(schema, "team", records, target_precision=0.8, minimum_answered=30)
definition = {
    "name": "ticket",
    "attributes": [
        {"name": "product", "role": "observed"},
        {"name": "summary", "role": "observed"},
        {"name": "team", "role": "decided", "key_fields": choice["key_fields"]},
    ],
}
thresholds, decisions = gil.select_layers(
    gil.Schema.from_dict(definition), "team", records, target_precision=0.8
)
```

`key_fields` is `None` when no narrower set did better than every attribute — leave it unset then.

The other settings follow the same pattern — choose, then set on the attribute: `select_key_concentrations` (set as
`key_concentrations`, after `key_fields`) and, for an attribute with a coarse level (`"coarse": {"prefix": 2}`),
`select_coarse` once its value thresholds are set.

A threshold trades precision for coverage — the share of lookups answered, `answer_rate` in each `chosen`, the word
selective prediction and automated coding use. Gil holds the precision (the target, at the bottom of an interval) and
reports the coverage it reaches.

When the target is a share of all the attribute's answers rather than a floor under every band of them, choose both
thresholds together with `rule="pooled"`: the pair that answers the most of the latest 5,000 lookups while all those
answers hold the target at the bottom of their Wilson interval at 1.645 standard errors (it reaches further back when
the attribute's current thresholds answered fewer than 1,000 of them). Each layer's `chosen` still reports that
layer's own precision and promise.

The bound is on the answers together, not on any group of them. Split by who sent the records in — the recalling firm,
the bug's reporter, the maintenance report's submitter, the hospital — on four public collections, `"pooled"` placed
19–22% of its answers with senders whose own precision was below the target beyond doubt (the top of a 95% Wilson
interval under 0.8), and the default `"every_band"` none. `"every_band"` does not look at the sender, though: when one sender's own
settlements are less reliable than the others', it answers below the target for that sender too. If you read a
promise per customer or per site, keep `"every_band"`, and count `gil.usage` over each sender's records to see it
hold.

```python
thresholds, _ = gil.select_layers(schema, "team", records, target_precision=0.8, rule="pooled")
```

Records settled together — several at once from one source — can be right or wrong together, and a promise taken on
records that batch mates span is then too high. Give each record its `batch` and `select_layers` adds `batches`: the
precision of thresholds chosen on half of the latest batches and counted on the other half (`across_batches`), the same
split by record (`across_records`), and how many answers a batch holds and how much they are right or wrong together
(`batch_size`, `correlation`, `design_effect`). Read `across_batches` as the promise only where `across_records` stands
above it and the design effect is well above one. The thresholds are chosen as before.

A promise bounds all the answers its thresholds give, not a batch's first record apart. Where a batch comes from one
source at once and its records show nearly the same things, the first record has nothing in memory to go on: on device
event reports its answers fell below the promise by 5.0 and 2.6 points in two quarters, while on food inspections and
vehicle complaints they held it. If a new batch's first record matters, count its answers apart with `usage`.

To hold a batch's first records to the target on their own (experimental), make the keeper with
`gil.Keeper(schema, ..., first_of_batch=True)` and open records with `gil.OpenRecord(schema, id, at, batch=...)`. The
keeper chooses the first records' thresholds on their own lookups and the rest on theirs — the choice's
`first_of_batch` holds them, `keeper.schema` carries them — and an open record whose batch has no other settled record
in the decider (`is_first_of_batch`) is answered at them, its promise `gil.first_of_batch_promise_of(...)`. A record
that names no batch always counts as a first record, so with no batches named it answers as before.

When a form's code lists change meaning, make the keeper with `gil.Keeper(schema, ..., restart_on_form_version=True)`
(experimental) and name each record's edition in its `form_version`: a record under a new edition starts the keeper
again from nothing — what came before leaves the decider, the replays and `keeper.schema`, records of an edition left
behind are not taken, and `keeper.form_version` says which edition is kept. On public food inspections across a revision
that renumbered the violation codes, a memory carried over promised 0.94 for the first thousand records after it and was
0.44 right, and 120,000 records later still answered 0.52 of lookups against 0.81 for one started anew; on public injury
reports across an edition that changed no code, starting anew answered 0.63 times as many over the first five thousand.
Name a new edition only when a value comes to mean something else.

A promise is an average over an attribute's values, and a value the layers keep taking for another can sit far below
it while the average holds. Pass `value_tails=True` to `gil.select_layers` or `gil.Keeper` (experimental), and the
thresholds chosen carry `value_tails`: for each value settled on at least 30 of the latest records asked about, how
many there were, the share the chosen thresholds answered with another value (`answered_otherwise`) and the value given
in its place most often (`given_instead`). On public food inspections and public device event reports, the values whose share was 0.20 or more on the first half of the records were mostly still at 0.10 or more on the second (1.00 and 0.88 of them, against 0.10 and 0.06 of the other values). A value with a large share is a
candidate for the attribute's `must_catch`. The report moves no threshold.

```python
records = gil.records({**r, "batch": r["import_id"]} for r in rows)
thresholds, _ = gil.select_layers(schema, "team", records, target_precision=0.8)
check = thresholds.get("batches")
```

## Answering a record while its values arrive

A decider holds the settled records; an open record asks it each time a value arrives or is settled, and hands back
the answers that changed. Times are 100-nanosecond ticks since the Unix epoch, UTC.

```python
decider = gil.Decider(schema, records)
record = gil.OpenRecord(schema, "t9", settled_at=now)
(team,) = record.observe(decider, "summary", "vpn drops every hour")
if team["answered"]:  # shown to the person, who keeps it
    record.settle(decider, "team", "Accept", now, value=team["candidates"][0]["value"])
else:  # nothing shown: the person types it
    record.settle(decider, "team", "Enter", now, value="network")
saved = record.snapshot()  # save it; gil.OpenRecord.reopen(schema, saved) picks it up again
```

Settle with the kind that says what the person saw: `Accept` / `Correct` for the answer shown, `Enter` with nothing
shown, `AcceptOther` / `CorrectOther` when the suggestion shown was not the decider's (your own model's, a default),
`Fill` when the application settled it with no person. Choosing thresholds and `gil.usage` read the kinds. Pass the
answer that was in view as `shown=` (with `promise=`, its layer's promise from the tuning) — or `suggested=` for a value
that was not the decider's — and the snapshot keeps them for `gil.usage`.

When an answer carries `needs_external`, no memory had evidence: ask your model about those (attribute, value) pairs
and pass its ranked values back with `record.offer_external(decider, "team", [{"value": ..., "probability": ...,
"trusted": ...}])`.

## Which field to ask for first (experimental)

When a decided attribute goes unanswered, asking the person for one observed attribute first may let the decider answer
it. `gil.AskPlanner` learns, from settled records in settlement order, what asking each attribute gained per character
typed, and advises an open record's missing attributes within a budget — a share of the typing asking every time would
take. The decider it learns beside may hold the record learned, but none settled after it.

```python
planner = gil.AskPlanner(schema, "team", ["product", "channel"], budget=0.3)
for settled in later:  # in settlement order, after the records the decider was made with
    decider.put(schema, gil.records([settled]))
    planner.learn(decider, settled)

values = {"summary": "vpn drops every hour"}
answer = decider.answer(schema, "t9", "team", values)  # writes nothing to memory
for advice in planner.advise(answer, values):  # the open attributes, highest gain first
    if advice["ask"]:
        ...  # ask the person for advice["attribute"]
```

## Keeping a tuning

How far to trust each layer is chosen by replaying the settled records. Put the choice on the schema with
`Schema.with_layers` — the key layer's `threshold` as `key_threshold`, the similar record layer's as `memory_threshold`
with its `similarity_floor` as `memory_floor` (`with_coarse` for a coarse level's) — and keep it as a tuning with what
each threshold promised:

```python
import json
from pathlib import Path

TARGET = 0.8


def tune(records):
    base = gil.Schema.from_dict(definition)
    layers, _ = gil.select_layers(base, "team", records, target_precision=TARGET, decisions=False)
    tuning = gil.capture_tuning(base.with_layers("team", layers), len(records), TARGET)
    return gil.record_promise(tuning, "team", layers)


Path("ticket.tuning.json").write_text(json.dumps(tune(records)))
```

A layer with no `chosen` threshold is left unset and does not answer the attribute. `capture_tuning` and
`tuning_stale` take the number of settled records, not the records. At start-up, `tuning_stale` says whether the kept
tuning still stands — the same package version, the same settings it rests on, and less than a tenth more or fewer records —
and `apply_tuning` puts it back on the schema; otherwise choose again:

```python
tuning = json.loads(Path("ticket.tuning.json").read_text())
base = gil.Schema.from_dict(definition)
if gil.tuning_stale(tuning, base, len(records)) is not None:
    tuning = tune(records)
schema = gil.apply_tuning(tuning, base)
```

While records keep arriving, `gil.Keeper` keeps the loop in one object. `settle` puts records into the decider and into
the next choice, `stale` says when the choice no longer stands, and `rechoose` replays only the records settled since the
last choice — the same thresholds as `select_layers` on all of them, without replaying them all again:

```python
keeper = gil.Keeper(gil.Schema.from_dict(definition), target_precision=TARGET)
decider = gil.Decider(keeper.schema)
keeper.settle(decider, records)
keeper.rechoose()

# ... as records are settled ...
keeper.settle(decider, gil.records([saved]))
if keeper.stale() is not None:
    keeper.rechoose()
schema = keeper.schema  # answer with this and the decider
```

It chooses for every decided attribute that takes one value, and for an attribute with a coarse level the level's
thresholds too, on the same replay — as `select_coarse` would with the value thresholds just chosen. A record settled out of order, or
one taken out with `keeper.remove(decider, id)`, has the next choice replay from the start. Nothing is stored outside
the process: a keeper made at start-up replays the saved records once.

An answer does not carry its promise; the promise is its layer's, which `gil.promise_of` reads from the tuning by the
answer's `source` (`keeper.tuning` for a keeper's). A coarse class offered instead reads its promise with
`gil.coarse_promise_of` by the coarse candidate's `source` — the value's layer promised nothing for it.
Keep what was in view on the record you save — the value suggested, its layer and its promise. `gil.usage(records)`
counts, without content, how the answers shown were settled and the precision they realized — `right_weight /
judged_weight`, to read against `promise_sum / promised` — and a reopened record carries them back until the attribute
is settled again:

```python
decider = gil.Decider(schema, records)

values = {"product": "vpn", "summary": "drops again"}
answer = decider.answer(schema, "t9", "team", values)
shown = answer["candidates"][0]["value"] if answer["answered"] else None
# ... the person settles `settled` ...
record = {
    "id": "t9",
    "settled_at": now,
    "values": {**values, "team": settled},
    "settlements": {"team": "Enter" if shown is None else "Accept" if settled == shown else "Correct"},
}
if shown is not None:
    record |= {"suggested": {"team": shown}, "sources": {"team": answer["source"]}}
    if (promise := gil.promise_of(tuning, "team", answer["source"])) is not None:
        record["promises"] = {"team": promise}
saved.append(record)

counts = gil.usage(gil.records(saved))["team"]
print(counts["right_weight"] / counts["judged_weight"], "against", counts["promise_sum"] / counts["promised"])
```

In operations terms: the automation rate is `answered / decisions`, the error rate of the answers one minus the
realized precision above, the acceptance rate `accepted / answered` and the override rate `(corrected + rejected) /
answered`. Acceptance and overrides are read off answers people saw; put the realized precision, which counts unseen
checks at their weight, beside the promise. A promise is the bottom of a Wilson interval at one standard error — about
one chance in six that the precision it holds is below it (one-sided 84%); `rule="pooled"` holds its pair at 1.645,
one-sided 95%.

**Status: early.** The API may change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/main/crates/gil/CHANGELOG.md) with what to do about it.

Apache-2.0 — see `LICENSE`.
