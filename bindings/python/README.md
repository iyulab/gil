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

**Status: early.** The API may change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/rust-core/crates/gil/CHANGELOG.md) with what to do about it.

Apache-2.0 — see `LICENSE`.
