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
`Fill` when the application settled it with no person. Choosing thresholds and `gil.usage` read the kinds.

When an answer carries `needs_external`, no memory had evidence: ask your model about those (attribute, value) pairs
and pass its ranked values back with `record.offer_external(decider, "team", [{"value": ..., "probability": ...,
"trusted": ...}])`.

## Keeping a tuning

`gil.capture_tuning(schema, settled, target_precision)` keeps the chosen settings (`gil.record_promise` adds what each
layer replay promised), `gil.apply_tuning` puts them back on a schema, and `gil.tuning_stale` says when to choose
again — another package version, a changed setting, or a tenth more (or fewer) settled records. `gil.usage(decisions)`
counts, without content, how the answers shown were settled and the precision they realized against the promise.

**Status: early.** The API may change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/rust-core/crates/gil/CHANGELOG.md) with what to do about it.

Apache-2.0 — see `LICENSE`.
