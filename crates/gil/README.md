# Gil (`iyulab-gil`)

Gil answers the decided attributes of a record from the records people have already settled, and chooses — by
replaying them in the order they were settled — how far each answer can be trusted and how often a trusted one will
be right. A record can be a form being filled in, an issue waiting for its component, a log line waiting for its risk
class, a patent waiting for its group: the core knows attributes and settled values, not forms.

This crate is its decision core, with no I/O. It is published as `iyulab-gil` and imported as `gil`
(`cargo add iyulab-gil`, then `use gil::…`). It writes the language-neutral goldens (`tests/fixtures/core` in the repository)
that the .NET library on NuGet, `Gil`, is checked against — `Gil` runs on this core through its C ABI.

**Status: early.** The API may still change within 0.x — each change is listed in
[CHANGELOG.md](https://github.com/iyulab/gil/blob/main/crates/gil/CHANGELOG.md) with what to do about it.

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

## Choosing the target precision

A threshold trades precision for coverage — the share of lookups answered, `answer_rate`, the word selective
prediction and automated coding use. Gil holds the precision (the target, at the bottom of an interval) and reports
the coverage it reaches.

`replay::Choice::target_precision` is yours to set, and it follows from what a wrong answer costs. Count a value a
person enters with no suggestion as 1, a right answer as nothing, and a wrong one — noticed, undone and entered again —
as ρ. An answer right with probability p then costs ρ(1 − p) against the 1 of asking, so answering pays only above a
precision of 1 − 1/ρ: 0.5 where a wrong value costs two entries, 0.8 at five, 0.9 at ten, 0.95 at twenty. Below that
line a threshold that keeps its promise still loses — each answer it adds costs more than it saves. Replaying a public
stream of biological sample records with costs counted this way, thresholds chosen for 0.8 answered right 0.88 of the
time — a loss once a wrong value costs ten entries; chosen for a target at or above 1 − 1/ρ, they cost less than asking
at every ρ from 2 to 20. When you do not know ρ, time how long undoing a wrong value takes against entering one.

A target can be read two ways. By default (`replay::ThresholdRule::EveryBand`, the default `rule` of
`replay::LayerOptions`) each layer's threshold is the lowest down to which every band of its answers meets the target —
a floor under every kind of answer. When the target is a share of all the attribute's answers instead, choose with
`replay::select_layers` and `LayerOptions { rule: ThresholdRule::Pooled, .. }`: the two thresholds together, as the pair
that answers the most of the attribute's latest `POOLED_RECENT_LOOKUPS` lookups while all those answers hold the target
at the bottom of their Wilson interval at `POOLED_STANDARD_ERRORS` (1.645) standard errors. The bound is on the answers
together, not on any group of them. Split by who sent the records in — the recalling firm, the bug's reporter, the
maintenance report's submitter, the hospital — on four public collections, `Pooled` placed 19–22% of its answers with
senders whose own precision was below the target beyond doubt (the top of a 95% Wilson interval under 0.8), and
`EveryBand` none. `EveryBand` does not look at the sender, though: when one sender's own settlements are less reliable
than the others', it answers below the target for that sender too. If you read a promise per customer or per site,
choose `EveryBand`, and count `usage` over each sender's decisions to see it hold.

Records settled together — several at once from one source, alike in what they show — can be right or wrong together,
and the replay then counts more independent answers than there are. Name each record's batch in `Record::batch` and
`select_layers` reports, in `LayerThresholds::batches`, what the batches among the latest `RECENT_RECORDS` records do:
the precision of thresholds chosen on half of them, counted on the other half, split by batch (`across_batches`) and
split by record (`across_records`), with how many answers a batch holds, how much they are right or wrong together and
the design effect of the two. Where `across_records` is above `across_batches` beyond chance, read `across_batches` as
the closer promise. The check moves no threshold. On public records, batches flattered a promise only where they were
large and right or wrong together: device event reports sent many at a time by one manufacturer (design effect about 7)
by 1.6 points; food inspections (1.1) and vehicle complaints, whose small batches were right or wrong together as much
(1.5), by nothing.

## License

Apache-2.0 — see `LICENSE`.
