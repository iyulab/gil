"""The bindings hand back what the core computes: the decision record golden, read through Python."""

from __future__ import annotations

import json
from datetime import datetime, timedelta
from pathlib import Path

import gilfit
import pytest

CORE = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "core"
CHOSEN = 200
COUNTED = ["Enter", "Accept", "Correct", "Check"]


def lines(name: str) -> list[dict]:
    text = (CORE / name).read_text(encoding="utf-8")
    return [json.loads(line) for line in text.splitlines() if line]


def golden_form(stop_field: str) -> dict:
    """The golden's form with the settings its lines chose before the `layers` line of `stop_field`."""
    team = {"name": "team", "role": "judged"}
    code = {"name": "code", "role": "judged", "coarse": {"prefix": 2}}
    tags = {"name": "tags", "role": "judged", "multiple": True}
    targets = {"team": team, "code": code, "tags": tags}
    for line in lines("golden.jsonl"):
        if line["kind"] == "decision" or (
            line["kind"] == "layers" and line["field"] == stop_field
        ):
            break
        target = targets.get(line.get("field"))
        if target is None:
            continue
        if line["kind"] == "key_fields":
            target["key_fields"] = line["fields"]
        elif line["kind"] == "key_concentrations":
            target["key_concentrations"] = line["concentrations"]
        elif line["kind"] == "layers":
            target["key_threshold"] = (line["key"]["chosen"] or {}).get("threshold")
            memory = line["memory"]["chosen"] or {}
            target["memory_threshold"] = memory.get("threshold")
            target["memory_similarity_floor"] = memory.get("similarity_floor")
        elif line["kind"] == "key":
            target["key_threshold"] = (line["replay"]["chosen"] or {}).get("threshold")
    return {
        "name": "ticket",
        "fields": [
            {"name": "channel", "role": "observed"},
            {"name": "product", "role": "observed"},
            {"name": "summary", "role": "observed"},
            team,
            code,
            tags,
        ],
    }


def settled_again() -> list[dict]:
    """The first records, then forty of them settled again — every other one under a new id."""
    early = lines("records.jsonl")[:CHOSEN]
    last = datetime.fromisoformat(early[-1]["settled_at"][:26] + "+00:00")
    again = list(early)
    for i, record in enumerate(early[:40]):
        copy = dict(record)
        if i % 2 == 1:
            copy["id"] = f"{record['id']}-copy"
        at = last + timedelta(minutes=10 * (i + 1))
        copy["settled_at"] = at.strftime("%Y-%m-%dT%H:%M:%S.0000000+00:00")
        again.append(copy)
    return again


def as_golden(record: dict) -> dict:
    """A record in the golden's shape (lists in place of objects)."""
    key, similar = record["key"], record["similar"]
    return {
        "kind": "record",
        "field": record["field"],
        "record": record["record"],
        "order": record["order"],
        "weight": record["weight"],
        "key": None
        if key is None
        else [key["strength"], key["correct"], key["key"], key["caught"]],
        "similar": None
        if similar is None
        else [
            similar["score"],
            similar["correct"],
            similar["nearest"],
            similar["voted"],
        ],
        "settled": record["settled"],
        "settlement": record["settlement"],
        "probability": record["probability"],
        "coarse": None,
    }


def test_select_layers_writes_the_golden_decision_records() -> None:
    documents = gilfit.documents(settled_again())
    expected = [line for line in lines("records-out.jsonl") if line["kind"] == "record"]
    got = []
    for field in ("team", "code"):
        form = gilfit.Form.from_dict(golden_form(field))
        thresholds, records = gilfit.select_layers(
            form, field, documents, 0.8, 10, counting=COUNTED
        )
        assert set(thresholds) == {"key", "memory"}
        bare, none = gilfit.select_layers(
            form, field, documents, 0.8, 10, counting=COUNTED, records=False
        )
        assert none is None and bare == thresholds
        got += [as_golden(r) for r in records]
    assert len(got) == len(expected)
    for i, (a, b) in enumerate(zip(got, expected, strict=True)):
        # JSON numbers: the golden writes whole doubles without a fraction.
        assert json.loads(json.dumps(a), parse_int=float) == json.loads(
            json.dumps(b), parse_int=float
        ), f"record line {i + 1}"


def test_the_golden_layers_choice_comes_back() -> None:
    documents = gilfit.read_records(CORE / "records.jsonl")
    assert len(documents) == len(lines("records.jsonl"))
    golden = {
        line["field"]: line
        for line in lines("golden.jsonl")
        if line["kind"] == "layers"
    }
    first = gilfit.documents(lines("records.jsonl")[:CHOSEN])
    for field, line in golden.items():
        form = gilfit.Form.from_dict(golden_form(field))
        thresholds, _ = gilfit.select_layers(
            form, field, first, 0.8, 10, counting=COUNTED, records=False
        )
        for layer in ("key", "memory"):
            want = line[layer]["chosen"]
            have = thresholds[layer]["chosen"]
            assert (want is None) == (have is None), (field, layer)
            if want is not None:
                assert have["threshold"] == want["threshold"], (field, layer)
                assert have["promise"] == want["promise"], (field, layer)


def test_bad_input_is_a_value_error() -> None:
    with pytest.raises(ValueError, match="unknown setting 'colour'"):
        gilfit.Form.from_dict(
            {"name": "f", "fields": [{"name": "a", "role": "judged", "colour": 1}]}
        )
    with pytest.raises(ValueError, match="needs a role"):
        gilfit.Form.from_dict({"name": "f", "fields": [{"name": "a"}]})
    with pytest.raises(ValueError, match="settled_at"):
        gilfit.documents([{"id": "1", "settled_at": "yesterday", "values": {}}])
    form = gilfit.Form.from_dict(
        {"name": "f", "fields": [{"name": "a", "role": "observed"}]}
    )
    with pytest.raises(ValueError, match="not judged"):
        gilfit.select_layers(form, "a", gilfit.documents([]))
