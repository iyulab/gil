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
| `probabilities` | object of numbers | no | For an attribute settled as a `Check`, the probability it was drawn with |
| `arrival` | string array | no | The attributes with a value, in the order their current values arrived, oldest first |
| `batch` | string | no | The batch the record was settled in — records settled together, several at once from one source |

An entry left out, or given as `null`, reads as empty (`arrival` and `batch` as unknown). An entry the table does not
name does not read, and neither does a time out of range (`2025-02-29`, `24:00`, an offset past ±14:00): the line is
refused rather than half understood. Lines are written back with `settled_at` in ticks.

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

## What to keep, before Gil is in use

1. Write a line each time a person settles a record, with the time and every value as it stood — not only the ones Gil
   will be asked about later, since which attributes key a decided one is chosen from the records.
2. Give each decided attribute its kind. Where the screen showed a proposal, keep whether it was taken.
3. Keep the lines in a place the application can read back in full. A replay needs all of them; a store that keeps only
   the latest state of each record loses the order and the corrections.

Leave out what Gil does not need and the form should not carry: free text that holds personal data can stay out of the
line, and the attribute then simply has no value to learn from.
