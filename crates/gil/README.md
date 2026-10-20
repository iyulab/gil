# Gil (`iyulab-gil`)

Gil answers the decided attributes of a record from the records people have already settled, and chooses — by
replaying them in the order they were settled — how far each answer can be trusted and how often a trusted one will
be right. A record can be a form being filled in, an issue waiting for its component, a log line waiting for its risk
class, a patent waiting for its group: the core knows attributes and settled values, not forms.

This crate is its decision core, with no I/O. It is published as `iyulab-gil` and imported as `gil`
(`cargo add iyulab-gil`, then `use gil::…`). It reproduces the language-neutral goldens of the .NET library published
on NuGet as `Gil` (`tests/fixtures/core` in the repository) to the bit.

**Status: early.** The API may still change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/rust-core/crates/gil/CHANGELOG.md) with what to do about it.

## Vocabulary

| Type | What it is |
|---|---|
| `schema::Schema` | The attributes of a record, in order; its name scopes memory |
| `schema::Attribute` | One attribute: observed (read) or decided (answered), with the settings that govern its answers |
| `schema::Record` | Settled values, with how each decided attribute was settled (`SettlementKind`) |
| `decider::Decider` | Answers a record's decided attributes layer by layer, or abstains |
| `decider::Answer` | One attribute's answer: candidates, the layer that answered, whether it is trusted |
| `replay` | Replays settled records to choose thresholds, key attributes and coarse levels |

## What it does

- **Key memory** — a value settled alongside the same value of another attribute, weighted toward the more recent,
  answers a decided attribute when the key backing it is strong enough.
- **Lexical record memory** — the nearest settled records, compared by character n-grams with no model, vote for a
  value.
- **Threshold choice** — each layer's threshold is chosen by replaying the settled records: the lowest score down to
  which every band of answers met the target precision, with the promise a replay of that many answers can make.
- **Decision records** — every lookup a replay or an answer makes can be handed to a sink, so a host can recount a
  choice or follow a layer over time without reaching into the replay.

The core does no I/O: records come in as values and decision records go out through a callback. How they are stored
is the host's business. Text is compared after the normalization specified in
[docs/text-normalization.md](https://github.com/iyulab/gil/blob/main/docs/text-normalization.md).

With the `serde` feature (`iyulab-gil = { version = "…", features = ["serde"] }`), what a host keeps or sends on —
schemas, records, settlements, tunings, usage counts — implements `Serialize` and `Deserialize`. Field names are the
Rust ones; roles and policies are written in snake case (`"decided"`, `"confirm_required"`), settlement kinds by name
(`"Accept"`), times in ticks; an attribute's settings left out take `Attribute::new`'s defaults, and an unknown one does
not read.

Python bindings are published on PyPI as `iyulab-gil` (`pip install iyulab-gil`, then `import gil`; Python 3.11 or
later).

## License

Apache-2.0 — see `LICENSE`.
