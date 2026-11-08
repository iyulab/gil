# Settled records

Gil learns from the records people have settled and chooses how far to trust its answers by replaying them. This page
specifies the line those records are kept in — the form the Rust core reads with its `serde` feature, .NET's
`SettledRecords.FromJsonLines` and Python's `gil.read_records` — and says what each entry is for, so that an
application can start keeping them before it uses Gil at all. On the day it does, the records already kept are what
the first thresholds are chosen from.

## The line

One JSON object per line, one line per settlement:

```json
{"id":"t1042","settled_at":17611488000000000,"values":{"product":"vpn","summary":"drops every hour","team":"network"},"settlements":{"team":"Accept"}}
```

| Entry | Type | Required | What it is |
|---|---|---|---|
| `id` | string | yes | The record's id, unique within its form. A record settled again is written again, with the same id and the later time |
| `settled_at` | integer or string | yes | When it was settled: 100-nanosecond ticks since the Unix epoch, UTC (.NET `DateTimeOffset.UtcTicks` − `DateTimeOffset.UnixEpoch.UtcTicks`), or the time as text `yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM` (.NET's round-trip `"O"` format) |
| `values` | object of strings | no | The value of each attribute that takes one, observed and decided alike, as it was when the record was settled |
| `sets` | object of string arrays | no | The values of each attribute that takes several |
| `settlements` | object of kinds | no | How each decided attribute was settled — see below |
| `probabilities` | object of numbers | no | For an attribute settled as a `Check`, the probability it was drawn with — above 0 and at most 1; a line with one outside that is refused |
| `arrival` | string array | no | The attributes with a value, in the order their current values arrived, oldest first |
| `batch` | string | no | The batch the record was settled in — records settled together, several at once from one source |
| `suggested` | object of strings | no | For each decided attribute settled with a suggestion in view, the value suggested — Gil's answer, or what the application showed in its place (the kind says which); for a coarse level, the class shown. For a `Check`, Gil's answer that was kept from the person who settled it, which the check is judged against |
| `sources` | object of layer names | no | For each decided attribute settled with Gil's answer in view — or, for a `Check`, kept from view — the layer that gave it: `KeyMemory`, `SimilarRecord` or `Model` |
| `promises` | object of numbers | no | For each decided attribute settled with Gil's answer in view — or, for a `Check`, kept from view — the precision promised for it when it was given (the tuning's promise for its layer), in [0, 1] |
| `scores` | object of numbers | no | For each decided attribute settled with an answer from a judge outside Gil in view (its `sources` entry is `Model`), the score the judge gave that answer when it was shown, on the judge's own scale — higher means surer; it is not a probability and is read for no other layer (experimental) |
| `settle_ms` | object of integers | no | For each decided attribute, the milliseconds from when the person reached it — its suggestion came into view, or it was reached with nothing in view — to when it was settled |
| `open_ms` | integer | no | The milliseconds the record was open, from opening it to this settlement |
| `settled_by` | object of strings | no | For each decided attribute, who settled it, as an opaque id the application gives — one person, one id; never a name |
| `promise_shown` | object of objects | no | For each decided attribute settled with Gil's answer and its promise given, `{"shown": …, "probability": …}`: whether the promise was displayed beside the answer, and the probability it was drawn to be displayed with — fixed before the draw; 1 for an application that always displays it, 0 for one that never does. Read only where the attribute has a promise and was not settled as a `Check` |
| `form_version` | string | no | The version of the form the record was settled on: the edition of the code lists its decided attributes are settled from, as the application identifies it |

An entry left out, or given as `null`, reads as empty (`arrival`, `batch` and the last seven as unknown). An entry the
table does not name does not read, and neither does a time out of range (`2025-02-29`, `24:00`, an offset past
±14:00), a promise outside [0, 1], a layer other than the three, a negative number of milliseconds or a promise
displayed at probability 0 (or not displayed at 1): the line is refused
rather than half understood. Lines are written back with `settled_at` in ticks.

The last seven entries are read from `Gil` 0.27.0 and `iyulab-gil` 0.9.0; earlier versions refuse a line that carries
them, so write them for a reader that knows them. `scores` is newer still: it is read from `Gil` 0.28.0 and `iyulab-gil` 0.10.0,
and earlier versions refuse a line that carries it. `promise_shown` is read from `Gil` 0.30.0 and `iyulab-gil` 0.12.0;
earlier versions refuse a line that carries it.

Lines may come in any order: a replay orders them by `settled_at`, then by `id`. A blank line is skipped.

## Why the time and the kind matter

- **`settled_at`** is what the replay runs on. Each record is asked about with only the records settled before it in
  memory, so a threshold is chosen on answers that could have been given at the time. A time that is not when the
  record was settled — the time it was imported, say — puts later records in memory before earlier ones, and the
  promise flatters the layers that learned from them.
- **`settlements`** says what a person did with an answer in view, and it decides what a record teaches:

  | Kind | What happened |
  |---|---|
  | `Accept` | The suggested value was taken as it was |
  | `Correct` | A value other than the suggested one was settled |
  | `Reject` | The suggestion was turned down and no value settled |
  | `Revert` | The attribute's earlier settlement was undone |
  | `Restore` | A saved value put back when a record was reopened |
  | `Set` | The values of an attribute that takes several were settled |
  | `Enter` | A person settled a value with no suggestion in view |
  | `Fill` | The application settled the value on its own — memory does not learn it, and choosing thresholds does not ask about it |
  | `Check` | A value settled again by someone who did not see the answer, drawn at random with the probability in `probabilities` — choosing thresholds counts it as one ÷ that probability decisions |
  | `AcceptOther` | A person kept a value suggested by something other than Gil — the application's own model, a rule, a default |
  | `CorrectOther` | A person settled another value than one suggested by something other than Gil |

  An attribute with no kind is a settlement whose kind is not known: it counts while thresholds are chosen over every
  settlement, and drops out once the choice names the kinds it counts (`counting`). Before Gil is in use there is no
  suggestion in view, so a person's own entry is `Enter`, and a value kept or changed from what the application
  proposed is `AcceptOther` or `CorrectOther`. A value no person looked at is `Fill`: it must not teach memory, or Gil
  learns to repeat whatever filled it.
- **`batch`** says which records were settled together. Records of one batch tend to be right or wrong together, and a
  promise chosen on them can be optimistic; a layers replay counts by how much when the latest records name their
  batch.
- **`arrival`** says which values were there when a decided attribute was settled. Without it, the observed values
  count as there from the start and the decided ones as settled in the form's order — right for a form filled top to
  bottom, optimistic for one where a later value is typed after the decided one was chosen.

## What was in view, what it cost, who settled it

The replays of Gil's own layers choose thresholds from `settled_at`, the values and the kinds; none of them reads the
last seven entries yet (`replay::scored_decisions` reads `suggested`, `sources` and `scores`, for a judge outside Gil). They are kept because a record cannot be given them afterwards, and each answers a question the others cannot:

- **`suggested`, `sources` and `promises`** — whether the promise held for the answers people actually saw, answer by
  answer and layer by layer, rather than for the answers a replay would have given; the usage counts (`Usage.Count`,
  `gil.usage`, `usage`) read them from the saved records. With `Check` settlements they also say how far a suggestion
  in view moves what people settle: the usage counts keep apart a check that was the decision's first settlement (the
  answer kept from view — `first_checks`) and one that came after it on the same id (an audit of the settler —
  `audits`, counted there only — the decision was counted at its first settlement). Which came first is read from `settled_at` among the records given, so counting by period means passing
  the records of the decisions first settled in it, their later audits included.
- **`scores`** — how sure a judge outside Gil was of the answer a person saw, so that a threshold and a promise for that
  judge can be chosen from the saved records alone. `replay::scored_decisions` turns the records settled with the
  judge's answer in view into the decisions `select_score_threshold` takes; a case settled as `Accept`, `Correct` or
  `Check` counts, a check as one over the probability it was drawn with. The score is kept as the judge gave it, on its
  own scale, because a record cannot be given it afterwards.
- **`settle_ms` and `open_ms`** — what settling costs a person, and whether it falls as settled records accumulate and
  more is left to Gil. A monotonic clock's difference is enough; the wall clock is `settled_at`.
- **`settled_by`** — whether settlers differ in how often they are right, which a promise counted over everyone hides.
  An opaque id is all it needs: the same person, the same id. One settlement per record is not enough to tell: a
  settler who agrees less often may be wrong more often, or may be given the harder records, and nothing in the
  records says which. What tells them apart is records settled twice — drawn at random, settled again by a second
  person who sees neither the answer nor the first settlement, written as a `Check` under the same id. Between them,
  with three settlers or more, how often each pair agrees gives each settler's rate of being right.
- **`settled_by`** is kept for each attribute because one record is often settled by more than one person — a clerk
  enters some attributes, a reviewer others — and a settler's rate of being right is a rate over the attributes they
  settled.
- **`promise_shown`** — whether displaying the promise beside an answer changes what people settle. An answer shown
  with "right 9 times in 10" beside it may be kept more readily than the same answer shown alone, and the promise then
  looks kept partly because it was displayed. Drawing at random, with a known probability fixed before the draw,
  whether each promise is displayed lets the two groups be compared on the same attribute; recording only the outcome
  could not tell a random draw from an application that displays promises on some screens and not others. The usage
  counts give the accepted and corrected answers with and without the promise displayed, per draw probability
  (`promise_display`).
- **`form_version`** — which records were settled on which version of a form. Give it a new value when a revision
  makes a value mean something else: a code renumbered, two codes merged into one, one split into several. Do not
  change it when only a label, a description or the order of the form changes. Records settled before a revision that
  renumbers codes answer the new form wrongly: on public food inspection records whose violation numbers were all
  reassigned, memory that went on across the revision answered 44% of the first 1,000 new records' answers right where
  94% was promised, and once its promise had come down it still answered a third less often than a fresh memory, for
  the year and a half the records ran. Starting
  memory again on a revision that changed nothing costs answers instead — on public injury product codes, 0.63 times
  as many in the first 5,000 records. Start from the records of the new version when the values changed meaning, and
  only then.

## What to keep, before Gil is in use

1. Write a line each time a person settles a record, with the time and every value as it stood — not only the ones Gil
   will be asked about later, since which attributes key a decided one is chosen from the records.
2. Give each decided attribute its kind. Where the screen showed a proposal, keep whether it was taken.
3. Keep the lines in a place the application can read back in full. A replay needs all of them; a store that keeps only
   the latest state of each record loses the order and the corrections.
   An export of the latest state also replays values that were not there when the record was settled. On public bug
   reports whose product, keywords and other attributes were edited after filing, thresholds replayed on the exported
   values promised 0.92, and their answers when asked with the values as filed were 0.84 right; replayed on the values
   as filed, they promised 0.95, were 0.93 right, and answered fewer records.

Leave out what Gil does not need and the form should not carry: free text that holds personal data can stay out of the
line, and the attribute then simply has no value to learn from.
