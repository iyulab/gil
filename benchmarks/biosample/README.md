# NCBI BioSample: an association-rule recommender and Gil

This benchmark compares Gil with the value recommender of the CEDAR metadata workbench — an association-rule
recommender — on the same public records and the same information. Both suggest the value of one field of a
BioSample record from the record's other fields. The comparison asks two things: how well the recommender ranks values
(mean reciprocal rank, MRR) under a random split and under a split in submission order, and how many values each can
answer while holding a target precision on records that come later.

## Summary

On NCBI BioSample human samples (the first record of each submission batch — one owner's records on one day; six
fields — tissue, sex, cell line, cell type and disease as in CEDAR's evaluation, age in place of ethnicity; the 21,142
records with at least three of them filled), we re-implemented the association-rule recommender from its published
source and compared it with Gil on the same information — the other five fields — training on the first 85% in
submission order and testing on the last 15%. A random 85/15 split did not overstate the recommender's MRR here (0.316
vs 0.309 in time order). The difference is in what each can promise: with a score threshold chosen on a random split
for 90% precision, the recommender answered 160 test lookups at 82.5%; Gil, choosing its thresholds by replaying the
records in the order they were settled and narrowing the fields that key each one, answered 1,065 at 95.6%. At 80% the
recommender kept its target (489 answers at 80.8%) and Gil answered 1,344 at 93.2%; at 95% the recommender found no
threshold that reached it. Mining the recommender's rules again as test records settle, as Gil keeps learning from
them, raised its answers at 80% only to 577 (from 489, at 78.9%), and no recommender arm kept a target by the lower
end of its interval — the end Gil states its own promise by.

## What is compared

**Data.** The NCBI BioSample full dump (`biosample_set.xml.gz` from <https://ftp.ncbi.nlm.nih.gov/biosample/>), as
downloaded on 2026-09-27: 59,703,237 records, 1,505,698 of them in the `Human.1.0` package. NCBI replaces the dump as
records arrive, so a later download gives a longer stream and different numbers.

**Stream.** `prepare.py` keeps the first record — earliest `submission_date`, then lowest `id` — of each submission
batch, the records one owner submitted on one day: 30,375 records, in submission order. Records within a batch are
usually filled alike, so keeping all of them would let both methods answer a batch from its own earlier records. The
six fields are taken by their harmonized attribute names: `tissue`, `sex`, `age`, `cell_line`, `cell_type`,
`disease` — the harmonized attributes filled most often among those records, leaving out names, identifiers, providers,
sample types, dates, places and free text, and any field with more distinct values than half its filled records. A value is trimmed and lower
cased; values that mean "no value" (`not applicable`, `missing`, `unknown` …) are dropped, by the list the LAFF
replication package uses for the same BioSample package (Belgacem et al., *A Machine Learning Approach for Automated
Filling of Categorical Fields in Data Entry Forms*, ACM TOSEM 2022, [doi:10.1145/3533021](https://doi.org/10.1145/3533021)).
Every other non-empty attribute is kept as the record's context.

**Split.** `compare.py` keeps the 21,142 records with at least three of the six fields filled, as CEDAR's evaluation
does. The first 17,970 (85%) in submission order are for training and the last 3,172 for testing. A test lookup asks for
one filled field of a test record, given the record's other filled fields among the six: 11,347 lookups.

**The recommender.** CEDAR's value recommender (Martínez-Romero et al., *Using association rule mining and ontologies
to generate metadata recommendations from multiple biomedical databases*, Database 2019, baz059), re-implemented from
its published source (`metadatacenter/cedar-valuerecommender-server`): rules mined by Apriori with support of at least
5 records and confidence of at least 0.3, one item as consequent and at most four as antecedent. A value's score is,
over the rules for the field whose antecedent shares an item with the context, the best Jaccard similarity of
antecedent and context times the rule's confidence; values scoring zero are dropped. To answer at a target precision it
takes the lowest threshold on its top score at which at least 10 answers hold the target on a calibration split of the
training records — 85/15 at random (`recommender-tau-random`) or in submission order (`recommender-tau-time`) — and
answers a test lookup when its top score reaches that threshold. The `-online` arms keep those thresholds and mine
the rules again from every record settled so far each time 100 more test records have settled, so the recommender also
learns during the test records.

**Gil.** One schema per field, the field decided and the other five observed. `select_layers` chooses the key and
similar-record thresholds by replaying the training records in submission order; the test lookups are answered at those
thresholds from the decision records of a replay of all records, in which each record is asked about before it is
settled — so Gil keeps learning from settled test records, as it would in use, while the recommender stays as trained.
`gil-key-fields` first narrows the fields that key each decided field with `select_key_fields` on the training records
(at least 30 answers). `gil-context` also observes the owner and every other attribute named in at least 50 records;
`gil-context-key-fields` narrows those too.

A target is **held** unless the upper end of the answers' 95% Wilson interval falls below it, and **kept** when the
interval's lower end reaches it — the sense in which Gil states its own promise.

## Running it

Python 3.11 or later; `prepare.py` needs only the standard library, `compare.py` needs `numpy` and `iyulab-gil`.

```
curl -O https://ftp.ncbi.nlm.nih.gov/biosample/biosample_set.xml.gz
uv run python benchmarks/biosample/prepare.py biosample_set.xml.gz --out human.jsonl.gz
uv run --with numpy --with iyulab-gil==0.8.0 python benchmarks/biosample/compare.py human.jsonl.gz --out results-0.9.json
uv run --with numpy --with iyulab-gil==0.8.0 python benchmarks/biosample/compare.py human.jsonl.gz --target 0.8 --out results-0.8.json
uv run --with numpy --with iyulab-gil==0.8.0 python benchmarks/biosample/compare.py human.jsonl.gz --target 0.95 --out results-0.95.json
```

`prepare.py` reads the 4.9 GB dump once (about 12 minutes on a recent laptop); `compare.py` takes a few minutes per target. The
results file holds every arm's answers by field and layer, the thresholds chosen and the fields chosen to key each
field.

## Results

Measured with `iyulab-gil` 0.8.0 on the 2026-09-27 dump (0.7.0 gave the same numbers).

**Ranking (MRR).** The recommender's MRR on the test lookups, with the context the record's other filled fields:

| Split | MRR | Top value right | Most frequent value |
|---|---|---|---|
| Random 85/15 | 0.316 | 0.248 | 0.190 |
| Submission order 85/15 | 0.309 | 0.240 | 0.175 |

The difference, +0.007, has a 95% interval of −0.004 to +0.019 (2,000 bootstrap resamples of the test records): a
random split did not overstate the recommender here.

**Answers at a target precision.** Answers to the 11,347 test lookups, and their precision; ✓ marks a target kept,
✗ a target not held, and no mark a target held but not kept.

| Arm | Target 0.8 | Target 0.9 | Target 0.95 |
|---|---|---|---|
| `recommender-tau-random` | 489 · 0.808 | 160 · 0.825 ✗ | none |
| `recommender-tau-random-online` | 577 · 0.789 | 193 · 0.819 ✗ | none |
| `recommender-tau-time` | 168 · 0.768 | none | none |
| `recommender-tau-time-online` | 215 · 0.809 | none | none |
| `gil` | 1,457 · 0.877 ✓ | 748 · 0.969 ✓ | 698 · 0.976 ✓ |
| `gil-key-fields` | 1,344 · 0.932 ✓ | 1,065 · 0.956 ✓ | 952 · 0.961 |
| `gil-context` | 2,187 · 0.829 ✓ | 1,092 · 0.872 ✗ | 741 · 0.955 |
| `gil-context-key-fields` | 1,739 · 0.915 ✓ | 1,364 · 0.943 ✓ | 1,130 · 0.958 |

"none" means no threshold reached the target on the calibration split, so the arm answered nothing.

- The threshold that broke its promise is the one chosen on a random split: chosen in submission order, the
  recommender's threshold at 0.9 answers nothing rather than answer below the target.
- At 0.9, Gil answers `sex` and `tissue` almost alone (568 and 172 of `gil`'s 748); narrowing the fields that key `sex`
  to `cell_line` and `tissue` raises its answers to 847.
- Learning during the test records is not what separates the two: mined again every 100 settled records, the
  recommender answers 18–28% more at 0.8 and 21% more at 0.9, while Gil's arms answer 2.3 to 3.8 times as many as
  the better online arm at 0.8 and 3.9 to 7.1 times at 0.9.
- At 0.95, where the recommender answers nothing, three of Gil's arms are held but not kept: their precision
  (0.955–0.961) sits on the target, with an interval wide enough to reach below it.
- `gil-context` falls short at 0.9 because values of fields unrelated to the decision became keys: in the training
  records an age written one way appeared with a single cell type, and in the test records with others. Narrowing the
  key fields with `select_key_fields` is what keeps such a field from answering, as `gil-context-key-fields` shows.

## Limits

- One package and one dump. The stream is smaller than the one in CEDAR's evaluation (about 21 thousand records against
  157 thousand) and its fields differ (age in place of ethnicity), so the recommender's MRR is lower than published
  there; that it rises with the size of the context matches.
- Keeping one record per submission batch leaves the owner of little use as a key.
- The recommender is a re-implementation, checked against the published source for its score (Jaccard similarity times
  confidence, over every rule whose antecedent shares an item with the context), for dropping zero scores, and for not
  falling back to recommendations without context when none match. It differs where the difference favours it: a value
  takes its best-scoring rule rather than the first found, and the number of recommendations is not capped.
- The contrast at 0.9 — a threshold chosen on a random split falls short, Gil's holds — depends on the target: at 0.8
  the recommender held it as well, and at 0.95 it answered nothing. What holds at all three targets is how many
  lookups each answers at the same target.
- The online recommender mines its rules again every 100 settled test records, not after each one; mining after each
  would only add the records of the last block.
- "Held" is judged by the Wilson interval's upper end, so a target is called held when the answers cannot show it was
  missed; with few answers that is weak evidence. "Kept" asks the opposite, that the answers show it was reached.
