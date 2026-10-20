# Gil (`iyulab-gil`)

Gil suggests the judged fields of a form from the documents people have already settled, and chooses — by
replaying them in the order they were settled — how far each suggestion can be trusted and how often a trusted one will
be right.

This crate is its decision core, with no I/O: a Rust rebuild of the .NET library published on NuGet as `Gil`. It is
published as `iyulab-gil` and imported as `gil` (`cargo add iyulab-gil`, then `use gil::…`). It
reproduces that library's language-neutral goldens (`tests/fixtures/core` in the repository) to the bit.

**Status: early.** The API may still change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/rust-core/crates/gil/CHANGELOG.md) with what to do about it.

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
[docs/text-normalization.md](https://github.com/iyulab/gil/blob/main/docs/text-normalization.md).

Python bindings are published on PyPI as `iyulab-gil` (`pip install iyulab-gil`, then `import gil`; Python 3.11 or
later).

## License

Apache-2.0 — see `LICENSE`.
