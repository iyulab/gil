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


def test_results_carry_exactly_the_typed_keys() -> None:
    # The TypedDicts are the documented shape; a field added or dropped on the Rust side must show up here.
    form = gilfit.Form.from_dict(golden_form("team"))
    thresholds, records = gilfit.select_layers(
        form, "team", gilfit.documents(settled_again()), 0.8, 10, counting=COUNTED
    )
    assert set(thresholds) == set(gilfit.LayerThresholds.__annotations__)
    for layer in thresholds.values():
        assert set(layer) == set(gilfit.ThresholdReplay.__annotations__)
        for choice in (layer["chosen"], layer["most_precise"]):
            if choice is not None:
                assert set(choice) == set(gilfit.ThresholdChoice.__annotations__)
        for state in layer["keys"] or []:
            assert set(state) == set(gilfit.KeyState.__annotations__)
    assert records
    for record in records:
        assert set(record) == set(gilfit.DecisionRecord.__annotations__)
        if record["key"] is not None:
            assert set(record["key"]) == set(gilfit.KeyLook.__annotations__)
        if record["similar"] is not None:
            assert set(record["similar"]) == set(gilfit.SimilarLook.__annotations__)


def declared_form() -> dict:
    """The golden's form as declared, before any line chose a setting."""
    return {
        "name": "ticket",
        "fields": [
            {"name": "channel", "role": "observed"},
            {"name": "product", "role": "observed"},
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "judged"},
            {"name": "code", "role": "judged", "coarse": {"prefix": 2}},
            {"name": "tags", "role": "judged", "multiple": True},
        ],
    }


def same_replay(have: dict, want: dict, where: str) -> None:
    for side in ("chosen", "most_precise"):
        h, w = have[side], want[side]
        assert (h is None) == (w is None), (where, side)
        if w is not None:
            for name in ("threshold", "precision", "answered", "promise"):
                assert h[name] == w[name], (where, side, name)
    assert have["lookups"] == want["lookups"], where


def test_the_golden_key_fields_choice_comes_back() -> None:
    first = gilfit.documents(lines("records.jsonl")[:CHOSEN])
    form = gilfit.Form.from_dict(declared_form())
    wanted = {line["world"]: line for line in lines("key_fields.jsonl")}
    for world, typed_longest in (("ticket", 0), ("ticket-typed", 2)):
        want = wanted[world]
        have = gilfit.select_key_fields(
            form, "team", first, 0.8, 10, counting=COUNTED, typed_longest=typed_longest
        )
        assert have["key_fields"] == want["fields"], world
        same_replay(have["chosen"], want["chosen"], world)
        same_replay(have["all_fields"], want["all_fields"], world)
        assert [t["key_fields"] for t in have["trials"]] == [
            t["fields"] for t in want["trials"]
        ], world
        for i, (h, w) in enumerate(zip(have["trials"], want["trials"], strict=True)):
            same_replay(h["replay"], w["replay"], f"{world} trial {i}")
        if typed_longest:
            assert have["typed"] is not None
            assert len(have["typed"]["by_length"]) == len(want["typed"])
            for i, (h, w) in enumerate(
                zip(have["typed"]["by_length"], want["typed"], strict=True)
            ):
                same_replay(h, w, f"{world} typed {i + 1}")
        else:
            assert have["typed"] is None
    # The choice goes on the form as the field's key fields.
    keyed = declared_form()
    keyed["fields"][3]["key_fields"] = wanted["ticket"]["fields"]
    gilfit.Form.from_dict(keyed)


def test_key_fields_choice_carries_exactly_the_typed_keys() -> None:
    form = gilfit.Form.from_dict(declared_form())
    choice = gilfit.select_key_fields(
        form, "team", gilfit.documents(lines("records.jsonl")[:CHOSEN]), typed_longest=2
    )
    assert set(choice) == set(gilfit.KeyFieldsChoice.__annotations__)
    for trial in choice["trials"]:
        assert set(trial) == set(gilfit.KeyFieldsTrial.__annotations__)
        assert set(trial["replay"]) == set(gilfit.ThresholdReplay.__annotations__)
    assert choice["typed"] is not None
    assert set(choice["typed"]) == set(gilfit.TypedKeyThresholds.__annotations__)
    with pytest.raises(ValueError, match="several values"):
        gilfit.select_key_fields(form, "tags", gilfit.documents([]), typed_longest=1)
