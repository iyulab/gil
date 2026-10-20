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

**Status: early.** The API may change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gilfit/blob/rust-core/crates/gilfit/CHANGELOG.md) with what to do about it.

Apache-2.0 — see `LICENSE`.
