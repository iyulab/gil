# gilfit (Python)

gilfit suggests the judged fields of a form from the documents people have already settled, and chooses — by
replaying them in the order they were settled — how far each suggestion can be trusted and how often a trusted one will
be right.

This package binds its Rust decision core: replay settled documents to choose how far each layer's suggestions for a
form field can be trusted, and get back the decision records the choice rests on.

```sh
pip install gilfit
```

```python
import gilfit

form = gilfit.Form.from_dict(
    {
        "name": "ticket",
        "fields": [
            {"name": "product", "role": "observed"},
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "judged"},
        ],
    }
)
documents = gilfit.read_records(
    "settled.jsonl"
)  # one JSON object per line, in settlement order
thresholds, records = gilfit.select_layers(
    form, "team", documents, target_precision=0.8
)
print(thresholds["key"]["chosen"], len(records))
```

Before choosing thresholds, narrow the fields that key the judged field. A field that happened to settle with one
value for a while would otherwise key every later document that shares it:

```python
choice = gilfit.select_key_fields(form, "team", documents, target_precision=0.8, minimum_answered=30)
definition = {
    "name": "ticket",
    "fields": [
        {"name": "product", "role": "observed"},
        {"name": "summary", "role": "observed"},
        {"name": "team", "role": "judged", "key_fields": choice["key_fields"]},
    ],
}
thresholds, records = gilfit.select_layers(
    gilfit.Form.from_dict(definition), "team", documents, target_precision=0.8
)
```

`key_fields` is `None` when no narrower set did better than every field — leave it unset then.

**Status: early.** The API may change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gilfit/blob/rust-core/crates/gilfit/CHANGELOG.md) with what to do about it.

Apache-2.0 — see `LICENSE`.
