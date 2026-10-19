# gilfit (Python)

Python bindings for the [gilfit](https://github.com/iyulab/gilfit) decision core: replay settled documents to choose how
far each layer's suggestions for a form field can be trusted, and get back the decision records the choice rests on.

```python
import gilfit

form = gilfit.Form.from_dict({
    "name": "ticket",
    "fields": [
        {"name": "product", "role": "observed"},
        {"name": "summary", "role": "observed"},
        {"name": "team", "role": "judged"},
    ],
})
documents = gilfit.read_records("settled.jsonl")  # one JSON object per line, in settlement order
thresholds, records = gilfit.select_layers(form, "team", documents, target_precision=0.8)
print(thresholds["key"]["chosen"], len(records))
```

**Status: early, not yet published.** The API may change within 0.x.

Apache-2.0 — see `LICENSE`.
