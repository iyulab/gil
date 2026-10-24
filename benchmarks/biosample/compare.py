"""Compares an association-rule value recommender with Gil on the same BioSample stream.

    uv run --with numpy --with iyulab-gil==0.8.0 python benchmarks/biosample/compare.py human.jsonl.gz --out results.json
    uv run --with numpy --with iyulab-gil==0.8.0 python benchmarks/biosample/compare.py human.jsonl.gz --target 0.8 --out results-0.8.json

The stream is the one ``prepare.py`` writes. Records with at least three of the six fields filled are kept; the first
85% in submission order train and the last 15% test. Each test lookup asks for one filled field, given the other filled
fields of the same record.

Arms:
  recommender       The association-rule recommender of CEDAR's value recommender, re-implemented from its published
                    source (Martinez-Romero et al., Database 2019, baz059; metadatacenter/cedar-valuerecommender-server):
                    rules from Apriori with support >= 5 records, confidence >= 0.3, one consequent and at most four
                    antecedent items. A value's score is the best |A & C| / |A | C| x confidence over the rules for the
                    field whose antecedent A shares an item with the context C; with an empty context, support over the
                    largest support. Ties go to the rule with more support.
  recommender-τ     The recommender, answering only when its top score reaches a threshold: the lowest threshold whose
                    answers hold the target precision (at least 10 answers) on a calibration split of the training
                    records — 85/15 at random, or 85/15 in submission order.
  recommender-τ-online
                    The same thresholds, with the rules mined again from every record settled so far each time 100 more
                    test records have settled — learning during the test records as Gil does.
  gil               Gil's ``select_layers``: the other five fields observed; thresholds chosen by replaying the training
                    records in settlement order, then applied to the test records' decisions from a replay of all records.
  gil-context       Also observing the owner and every other attribute that appears in at least 50 records.
  ... -key-fields   The same, with the fields that key each decided field narrowed first by ``select_key_fields`` on the
                    training records.
"""

from __future__ import annotations

import argparse
import gzip
import json
import math
import random
import sys
from collections import Counter, defaultdict
from itertools import combinations
from typing import Any

import gil
import numpy as np

FIELDS = ["tissue", "sex", "age", "cell_line", "cell_type", "disease"]
SEED = 20261020
MIN_SUPPORT = 5
MIN_CONFIDENCE = 0.3
MAX_ANTECEDENT = 4
MIN_FILLED = 3
MIN_ANSWERED = 10
KEY_FIELDS_MINIMUM = 30
CONTEXT_MIN_RECORDS = 50
TRAIN = 0.85
# test records settled between two minings of the online recommender
RETRAIN_EVERY = 100
EPOCH_2000 = 946_684_800  # seconds since the Unix epoch; records settle one second apart from here


def read(path: str) -> list[dict]:
    with gzip.open(path, "rt", encoding="utf-8") as f:
        return [json.loads(line) for line in f]


# ---------------------------------------------------------------- the recommender


class Recommender:
    """Association-rule value recommender; an item is a (field, value) pair."""

    def __init__(self, docs: list[dict[str, str]], fields: list[str]) -> None:
        support: Counter[frozenset] = Counter()
        for d in docs:
            items = sorted(d.items())
            for size in range(1, min(len(items), MAX_ANTECEDENT + 1) + 1):
                for s in combinations(items, size):
                    support[frozenset(s)] += 1
        # rule = (consequent field, antecedent, consequent value, confidence, support)
        self.rules: list[tuple[str, frozenset, str, float, int]] = []
        self.by_item: dict[tuple[str, tuple[str, str]], list[int]] = defaultdict(list)
        self.prior: dict[str, list[tuple[str, int]]] = defaultdict(list)
        for s, sup in support.items():
            if sup < MIN_SUPPORT:
                continue
            if len(s) == 1:
                ((f, v),) = s
                self.prior[f].append((v, sup))
            for c in s:
                a = s - {c}
                if not a:
                    continue
                confidence = sup / support[a]
                if confidence >= MIN_CONFIDENCE:
                    i = len(self.rules)
                    self.rules.append((c[0], a, c[1], confidence, sup))
                    for item in a:
                        self.by_item[(c[0], item)].append(i)
        # baseline: the most frequent training value of each field
        counts: dict[str, Counter[str]] = {f: Counter() for f in fields}
        for d in docs:
            for f, v in d.items():
                counts[f][v] += 1
        self.majority = {
            f: counts[f].most_common(1)[0][0] if counts[f] else None for f in fields
        }

    def rank(self, field: str, context: dict[str, str]) -> list[tuple[str, float]]:
        """(value, score), best first; equal scores go to the value with more support."""
        best: dict[str, tuple[float, int]] = {}
        items = sorted(context.items())
        if not items:
            prior = self.prior.get(field, [])
            if not prior:
                return []
            top = max(sup for _, sup in prior)
            for v, sup in prior:
                best[v] = (sup / top, sup)
        else:
            # every rule for the field whose antecedent shares an item with the context, scored by the Jaccard
            # similarity of antecedent and context times its confidence; a value takes its best rule
            ctx = set(items)
            seen: set[int] = set()
            for item in items:
                for i in self.by_item.get((field, item), ()):
                    if i in seen:
                        continue
                    seen.add(i)
                    _, a, v, confidence, sup = self.rules[i]
                    shared = len(a & ctx)
                    score = shared / (len(ctx) + len(a) - shared) * confidence
                    old = best.get(v)
                    if old is None or (score, sup) > old:
                        best[v] = (score, sup)
        ranked = sorted(best.items(), key=lambda x: (-x[1][0], -x[1][1], x[0]))
        return [(v, s) for v, (s, _) in ranked if s > 0]


def reciprocal_rank(ranking: list[tuple[str, float]], truth: str) -> float:
    for i, (v, _) in enumerate(ranking, start=1):
        if v == truth:
            return 1.0 / i
    return 0.0


def lookups(docs: list[dict[str, str]]):
    """(record index, field, settled value, context = the record's other filled fields)."""
    for i, d in enumerate(docs):
        for f, v in d.items():
            yield i, f, v, {g: w for g, w in d.items() if g != f}


def choose_tau(scored: list[tuple[float, bool]], target: float) -> float | None:
    """The lowest top score whose answers at or above it hold ``target`` precision, with at least 10 answers."""
    scored = sorted(scored, key=lambda x: -x[0])
    best = None
    hit = 0
    for i, (s, ok) in enumerate(scored, start=1):
        hit += ok
        # cut only at the end of a run of equal scores
        if i < len(scored) and scored[i][0] == s:
            continue
        if i >= MIN_ANSWERED and hit / i >= target:
            best = s
    return best


def calibrated(
    train: list[dict[str, str]], split: str, target: float
) -> tuple[Recommender, float | None, int]:
    idx = list(range(len(train)))
    if split == "random":
        random.Random(SEED).shuffle(idx)
    cut = int(TRAIN * len(idx))
    model = Recommender([train[i] for i in idx[:cut]], FIELDS)
    scored = []
    for _, f, v, ctx in lookups([train[i] for i in idx[cut:]]):
        r = model.rank(f, ctx)
        if r:
            scored.append((r[0][1], r[0][0] == v))
    return model, choose_tau(scored, target), len(scored)


def thresholded(
    model: Recommender, tau: float | None, test: list[dict[str, str]]
) -> dict:
    if tau is None:
        return {"tau": None, "answered": 0, "correct": 0}
    n = ok = 0
    for _, f, v, ctx in lookups(test):
        r = model.rank(f, ctx)
        if r and r[0][1] >= tau:
            n += 1
            ok += r[0][0] == v
    return {"tau": tau, "answered": n, "correct": ok}


def thresholded_online(docs: list[dict[str, str]], cut: int, tau: float | None) -> dict:
    """``thresholded`` on the test records ``docs[cut:]``, mining the rules again from every record before each block
    of ``RETRAIN_EVERY`` test records."""
    if tau is None:
        return {"tau": None, "answered": 0, "correct": 0}
    n = ok = 0
    for start in range(cut, len(docs), RETRAIN_EVERY):
        model = Recommender(docs[:start], FIELDS)
        for _, f, v, ctx in lookups(docs[start : start + RETRAIN_EVERY]):
            r = model.rank(f, ctx)
            if r and r[0][1] >= tau:
                n += 1
                ok += r[0][0] == v
    return {"tau": tau, "answered": n, "correct": ok}


def mrr(
    model: Recommender, docs: list[dict[str, str]]
) -> tuple[np.ndarray, np.ndarray, dict]:
    """Per-record reciprocal rank sums and lookup counts (for the bootstrap), and the summary."""
    s = np.zeros(len(docs))
    n = np.zeros(len(docs))
    by_context: dict[int, list[float]] = defaultdict(list)
    base: list[float] = []
    top1: list[bool] = []
    for i, f, v, ctx in lookups(docs):
        r = model.rank(f, ctx)
        x = reciprocal_rank(r, v)
        s[i] += x
        n[i] += 1
        by_context[min(len(ctx), 4)].append(x)
        base.append(1.0 if model.majority.get(f) == v else 0.0)
        top1.append(bool(r) and r[0][0] == v)
    summary = {
        "mrr": float(s.sum() / n.sum()),
        "top1": float(np.mean(top1)),
        "majority_baseline": float(np.mean(base)),
        "lookups": int(n.sum()),
        "by_context_size": {
            str(k): [float(np.mean(v)), len(v)] for k, v in sorted(by_context.items())
        },
    }
    return s, n, summary


def bootstrap_delta(sa, na, sb, nb, reps: int = 2000) -> tuple[float, float]:
    """95% interval of MRR(a) − MRR(b), resampling test records."""
    rng = np.random.default_rng(SEED)
    d = np.empty(reps)
    for b in range(reps):
        ia = rng.integers(0, len(sa), len(sa))
        ib = rng.integers(0, len(sb), len(sb))
        d[b] = sa[ia].sum() / na[ia].sum() - sb[ib].sum() / nb[ib].sum()
    lo, hi = np.percentile(d, [2.5, 97.5])
    return float(lo), float(hi)


# ---------------------------------------------------------------- Gil


def gil_records(rows: list[dict], extra: list[str]) -> list[dict]:
    """The stream as settled records, one second apart in submission order."""
    out = []
    keep = set(extra)
    for t, row in enumerate(rows, start=1):
        values: dict[str, str] = {}
        if "owner" in keep and row["owner"]:
            values["owner"] = row["owner"]
        for name, value in row["context"]:
            if name in keep and name != "owner":
                values.setdefault(name, value)
        for f in FIELDS:
            if f in row["targets"]:
                values[f] = row["targets"][f]
        out.append(
            {
                "id": str(t),
                "settled_at": (EPOCH_2000 + t) * 10_000_000,
                "values": values,
            }
        )
    return out


def context_attributes(rows: list[dict]) -> list[str]:
    """The owner and every other attribute named in at least 50 records."""
    names: Counter[str] = Counter()
    for row in rows:
        names.update({n for n, _ in row["context"]})
    rest = sorted(
        (
            n
            for n, c in names.items()
            if c >= CONTEXT_MIN_RECORDS and n not in FIELDS and n != "owner"
        ),
        key=lambda s: s.encode("utf-16-le"),
    )
    return ["owner", *rest]


def answer(
    rec: dict, key_t: float | None, memory_t: float | None, floor: float | None
) -> tuple[str | None, str | None]:
    """The layers in order: the key layer (not caught, strength at its threshold), then the similar record vote (score
    at its threshold, not on a caught key, and with a voted answer's nearest record at the similarity floor)."""
    k = rec.get("key")
    if (
        k is not None
        and key_t is not None
        and not k["caught"]
        and k["strength"] >= key_t
    ):
        return k["value"], "key"
    s = rec.get("similar")
    if s is not None and memory_t is not None and s["score"] >= memory_t:
        caught = k is not None and k["caught"]
        floored = floor is not None and s["voted"] and s["nearest"] < floor
        if not caught and not floored:
            return s["value"], "similar"
    return None, None


def gil_arm(
    rows: list[dict], ntrain: int, extra: list[str], key_fields: bool, target: float
) -> dict:
    per_field = {}
    for field in FIELDS:
        observed = [f for f in FIELDS if f != field] + [
            e for e in extra if e not in FIELDS
        ]
        decided: dict[str, Any] = {"name": field, "role": "decided"}
        definition = {
            "name": "biosample",
            "attributes": [{"name": n, "role": "observed"} for n in observed]
            + [decided],
        }
        records = gil_records(rows, extra)
        narrowed = None
        if key_fields:
            choice = gil.select_key_fields(
                gil.Schema.from_dict(definition),
                field,
                gil.records(records[:ntrain]),
                target,
                KEY_FIELDS_MINIMUM,
            )
            narrowed = choice["key_fields"]
            if narrowed is not None:
                decided["key_fields"] = narrowed
        schema = gil.Schema.from_dict(definition)
        _, decisions = gil.select_layers(
            schema, field, gil.records(records), target, MIN_ANSWERED
        )
        chosen, _ = gil.select_layers(
            schema,
            field,
            gil.records(records[:ntrain]),
            target,
            MIN_ANSWERED,
            decisions=False,
        )
        kc, mc = chosen["key"]["chosen"], chosen["memory"]["chosen"]
        key_t = kc["threshold"] if kc else None
        memory_t = mc["threshold"] if mc else None
        floor = mc["similarity_floor"] if mc else None
        test = [
            r
            for r in decisions or []
            if int(r["record"]) > ntrain and r["settled"] is not None
        ]
        answers = [answer(r, key_t, memory_t, floor) for r in test]
        answered = [
            (a == r["settled"], layer)
            for r, (a, layer) in zip(test, answers, strict=True)
            if a is not None
        ]
        per_field[field] = {
            "key_fields": narrowed,
            "thresholds": {"key": key_t, "memory": memory_t, "floor": floor},
            "test_lookups": len(test),
            "answered": len(answered),
            "correct": sum(ok for ok, _ in answered),
            "by_layer": {
                layer: [
                    sum(1 for _, l in answered if l == layer),
                    sum(ok for ok, l in answered if l == layer),
                ]
                for layer in ("key", "similar")
            },
        }
    return per_field


# ---------------------------------------------------------------- report


def wilson(hit: int, n: int, z: float = 1.959964) -> tuple[float | None, float | None]:
    if n == 0:
        return None, None
    p = hit / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    r = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n))
    return (c - r) / d, (c + r) / d


def promise(answered: int, correct: int, target: float) -> dict:
    lo, hi = wilson(correct, answered)
    return {
        "answered": answered,
        "correct": correct,
        "precision": correct / answered if answered else None,
        "wilson95": [lo, hi],
        # held unless the interval's upper end falls below the target
        "held": None if hi is None else bool(hi >= target),
        # kept when the interval's lower end reaches it: the sense in which Gil states its own promise
        "kept": None if lo is None else bool(lo >= target),
    }


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("stream", help="the stream prepare.py wrote")
    ap.add_argument("--out", required=True, help="results (JSON)")
    ap.add_argument(
        "--target", type=float, default=0.9, help="target precision (default 0.9)"
    )
    args = ap.parse_args()
    target = args.target

    rows = read(args.stream)
    rows = [r for r in rows if sum(f in r["targets"] for f in FIELDS) >= MIN_FILLED]
    docs = [{f: r["targets"][f] for f in FIELDS if f in r["targets"]} for r in rows]
    n = len(docs)
    cut = int(TRAIN * n)
    test = docs[cut:]
    report: dict[str, Any] = {
        "records": n,
        "train": cut,
        "test": n - cut,
        "fields": FIELDS,
        "target": target,
        "test_lookups": sum(1 for _ in lookups(test)),
    }

    # MRR: random 85/15 against submission order 85/15
    idx = list(range(n))
    random.Random(SEED).shuffle(idx)
    random_model = Recommender([docs[i] for i in idx[:cut]], FIELDS)
    sa, na, random_mrr = mrr(random_model, [docs[i] for i in idx[cut:]])
    sb, nb, time_mrr = mrr(Recommender(docs[:cut], FIELDS), test)
    report["mrr"] = {
        "random_split": random_mrr,
        "time_split": time_mrr,
        "difference": random_mrr["mrr"] - time_mrr["mrr"],
        "difference_ci95": list(bootstrap_delta(sa, na, sb, nb)),
    }

    # answers at the target precision, on the time-ordered test records
    arms: dict[str, Any] = {}
    for split in ("random", "time"):
        model, tau, calibration = calibrated(docs[:cut], split, target)
        t = thresholded(model, tau, test)
        arms[f"recommender-tau-{split}"] = {
            "tau": t["tau"],
            "calibration_lookups": calibration,
        } | promise(t["answered"], t["correct"], target)
        print(f"recommender-tau-{split}-online ...", file=sys.stderr, flush=True)
        o = thresholded_online(docs, cut, tau)
        arms[f"recommender-tau-{split}-online"] = {
            "tau": o["tau"],
            "calibration_lookups": calibration,
            "retrain_every": RETRAIN_EVERY,
        } | promise(o["answered"], o["correct"], target)
    extra = context_attributes(rows)
    for name, observed, narrow in (
        ("gil", [], False),
        ("gil-key-fields", [], True),
        ("gil-context", extra, False),
        ("gil-context-key-fields", extra, True),
    ):
        print(f"{name} ...", file=sys.stderr, flush=True)
        per_field = gil_arm(rows, cut, observed, narrow, target)
        a = sum(e["answered"] for e in per_field.values())
        c = sum(e["correct"] for e in per_field.values())
        arms[name] = promise(a, c, target) | {"per_field": per_field}
    report["answers"] = arms

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=1)

    print(
        f"MRR  random {random_mrr['mrr']:.3f}  time {time_mrr['mrr']:.3f}  "
        f"difference {report['mrr']['difference']:+.3f} "
        f"[{report['mrr']['difference_ci95'][0]:+.3f}, {report['mrr']['difference_ci95'][1]:+.3f}]"
    )
    print(f"\ntarget {target}  test lookups {report['test_lookups']:,}")
    print(f"{'arm':<33}{'answered':>9}{'precision':>11}  wilson95         held   kept")
    for name, a in arms.items():
        p = "-" if a["precision"] is None else f"{a['precision']:.3f}"
        lo, hi = a["wilson95"]
        w = "-" if lo is None else f"{lo:.3f} - {hi:.3f}"
        print(
            f"{name:<33}{a['answered']:>9,}{p:>11}  {w:<16} {a['held']!s:<6} {a['kept']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
