# gilfit

The decision core of [gilfit](https://github.com/iyulab/gilfit): it suggests the judged fields of a form — a ticket's
team, a report's fault code — from the documents already settled, and chooses how far each suggestion can be trusted by
replaying those documents in the order they were settled.

**Status: early, not yet published.** This crate is a Rust rebuild of the .NET library published on NuGet as `Gil`. It
reproduces that library's language-neutral goldens (`tests/fixtures/core` in the repository) to the bit; its API may
still change within 0.x.

## What it does

- **Settled field memory** — a value settled alongside the same value of another field, weighted toward the more
  recent, answers a judged field when the key backing it is strong enough.
- **Lexical document memory** — the nearest settled documents, compared by character n-grams with no model, vote for a
  value.
- **Threshold choice** — each layer's threshold is chosen by replaying the settled documents: the lowest score down to
  which every band of answers met the target precision, with the promise a replay of that many answers can make.
- **Decision records** — every lookup a replay or a suggestion makes can be handed to a sink, so a host can recount a
  choice or follow a layer over time without reaching into the replay.

The core does no I/O: documents come in as values and records go out through a callback. How they are stored is the
host's business. Text is compared after the normalization specified in
[docs/text-normalization.md](https://github.com/iyulab/gilfit/blob/main/docs/text-normalization.md).

Python bindings live under `bindings/python` in the repository.

## License

Apache-2.0 — see `LICENSE`.
