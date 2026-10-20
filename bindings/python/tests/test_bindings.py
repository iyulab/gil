"""The bindings hand back what the core computes: the decision record golden, read through Python."""

from __future__ import annotations

import json
from datetime import datetime, timedelta
from pathlib import Path

import gil
import pytest

CORE = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "core"
CHOSEN = 200
COUNTED = ["Enter", "Accept", "Correct", "Check"]


def lines(name: str) -> list[dict]:
    text = (CORE / name).read_text(encoding="utf-8")
    return [json.loads(line) for line in text.splitlines() if line]


def golden_form(stop_field: str, stop_kind: str = "layers") -> dict:
    """The golden's schema with the settings its lines chose before the `stop_kind` line of `stop_field`."""
    team = {"name": "team", "role": "decided"}
    code = {"name": "code", "role": "decided", "coarse": {"prefix": 2}}
    tags = {"name": "tags", "role": "decided", "multiple": True}
    targets = {"team": team, "code": code, "tags": tags}
    for line in lines("golden.jsonl"):
        if line["kind"] == "decision" or (
            line["kind"] == stop_kind and line["field"] == stop_field
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
            target["memory_floor"] = memory.get("similarity_floor")
        elif line["kind"] == "key":
            target["key_threshold"] = (line["replay"]["chosen"] or {}).get("threshold")
    return {
        "name": "ticket",
        "attributes": [
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


def as_golden(record: dict, kind: str = "record") -> dict:
    """A record in the golden's shape (lists in place of objects)."""
    key, similar, coarse = record["key"], record["similar"], record["coarse"]
    return {
        "kind": kind,
        "field": record["attribute"],
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
        "coarse": None
        if coarse is None
        else [
            coarse["value_answered"],
            None
            if coarse["keys"] is None
            else [coarse["keys"]["strength"], coarse["keys"]["correct"]],
            None
            if coarse["vote"] is None
            else [
                coarse["vote"]["score"],
                coarse["vote"]["correct"],
                coarse["vote"]["nearest"],
                coarse["vote"]["voted"],
            ],
        ],
    }


def test_select_layers_writes_the_golden_decision_records() -> None:
    settled = gil.records(settled_again())
    expected = [line for line in lines("records-out.jsonl") if line["kind"] == "record"]
    got = []
    for attribute in ("team", "code"):
        schema = gil.Schema.from_dict(golden_form(attribute))
        thresholds, decisions = gil.select_layers(
            schema, attribute, settled, 0.8, 10, counting=COUNTED
        )
        assert set(thresholds) == {"key", "memory"}
        bare, none = gil.select_layers(
            schema, attribute, settled, 0.8, 10, counting=COUNTED, decisions=False
        )
        assert none is None and bare == thresholds
        got += [as_golden(r) for r in decisions]
    assert len(got) == len(expected)
    for i, (a, b) in enumerate(zip(got, expected, strict=True)):
        # JSON numbers: the golden writes whole doubles without a fraction.
        assert json.loads(json.dumps(a), parse_int=float) == json.loads(
            json.dumps(b), parse_int=float
        ), f"record line {i + 1}"


def test_the_golden_layers_choice_comes_back() -> None:
    records = gil.read_records(CORE / "records.jsonl")
    assert len(records) == len(lines("records.jsonl"))
    golden = {
        line["field"]: line
        for line in lines("golden.jsonl")
        if line["kind"] == "layers"
    }
    first = gil.records(lines("records.jsonl")[:CHOSEN])
    for attribute, line in golden.items():
        schema = gil.Schema.from_dict(golden_form(attribute))
        thresholds, _ = gil.select_layers(
            schema, attribute, first, 0.8, 10, counting=COUNTED, decisions=False
        )
        for layer in ("key", "memory"):
            want = line[layer]["chosen"]
            have = thresholds[layer]["chosen"]
            assert (want is None) == (have is None), (attribute, layer)
            if want is not None:
                assert have["threshold"] == want["threshold"], (attribute, layer)
                assert have["promise"] == want["promise"], (attribute, layer)


def test_bad_input_is_a_value_error() -> None:
    with pytest.raises(ValueError, match="unknown setting 'colour'"):
        gil.Schema.from_dict(
            {"name": "f", "attributes": [{"name": "a", "role": "decided", "colour": 1}]}
        )
    with pytest.raises(ValueError, match="needs a role"):
        gil.Schema.from_dict({"name": "f", "attributes": [{"name": "a"}]})
    with pytest.raises(ValueError, match="settled_at"):
        gil.records([{"id": "1", "settled_at": "yesterday", "values": {}}])
    with pytest.raises(ValueError, match="at least one decided attribute"):
        gil.Schema.from_dict(
            {"name": "f", "attributes": [{"name": "a", "role": "observed"}]}
        )
    with pytest.raises(ValueError, match="takes its candidates from 'b'"):
        gil.Schema.from_dict(
            {
                "name": "f",
                "attributes": [
                    {"name": "a", "role": "decided", "candidates_from": "b"}
                ],
            }
        )
    schema = gil.Schema.from_dict(
        {
            "name": "f",
            "attributes": [
                {"name": "a", "role": "observed"},
                {"name": "b", "role": "decided"},
            ],
        }
    )
    with pytest.raises(ValueError, match="not decided"):
        gil.select_layers(schema, "a", gil.records([]))


def test_results_carry_exactly_the_typed_keys() -> None:
    # The TypedDicts are the documented shape; an attribute added or dropped on the Rust side must show up here.
    schema = gil.Schema.from_dict(golden_form("team"))
    thresholds, records = gil.select_layers(
        schema, "team", gil.records(settled_again()), 0.8, 10, counting=COUNTED
    )
    assert set(thresholds) == set(gil.LayerThresholds.__annotations__)
    for layer in thresholds.values():
        assert set(layer) == set(gil.ThresholdReplay.__annotations__)
        for choice in (layer["chosen"], layer["most_precise"]):
            if choice is not None:
                assert set(choice) == set(gil.ThresholdChoice.__annotations__)
        for state in layer["keys"] or []:
            assert set(state) == set(gil.KeyState.__annotations__)
    assert records
    for record in records:
        assert set(record) == set(gil.DecisionRecord.__annotations__)
        if record["key"] is not None:
            assert set(record["key"]) == set(gil.KeyLook.__annotations__)
        if record["similar"] is not None:
            assert set(record["similar"]) == set(gil.SimilarLook.__annotations__)
        assert record["coarse"] is None


def declared_form() -> dict:
    """The golden's schema as declared, before any line chose a setting."""
    return {
        "name": "ticket",
        "attributes": [
            {"name": "channel", "role": "observed"},
            {"name": "product", "role": "observed"},
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "decided"},
            {"name": "code", "role": "decided", "coarse": {"prefix": 2}},
            {"name": "tags", "role": "decided", "multiple": True},
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
    first = gil.records(lines("records.jsonl")[:CHOSEN])
    schema = gil.Schema.from_dict(declared_form())
    wanted = {line["world"]: line for line in lines("key_fields.jsonl")}
    for world, typed_longest in (("ticket", 0), ("ticket-typed", 2)):
        want = wanted[world]
        have = gil.select_key_fields(
            schema,
            "team",
            first,
            0.8,
            10,
            counting=COUNTED,
            typed_longest=typed_longest,
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
    # The choice goes on the schema as the attribute's key fields.
    keyed = declared_form()
    keyed["attributes"][3]["key_fields"] = wanted["ticket"]["fields"]
    gil.Schema.from_dict(keyed)


def test_key_fields_choice_carries_exactly_the_typed_keys() -> None:
    schema = gil.Schema.from_dict(declared_form())
    choice = gil.select_key_fields(
        schema, "team", gil.records(lines("records.jsonl")[:CHOSEN]), typed_longest=2
    )
    assert set(choice) == set(gil.KeyFieldsChoice.__annotations__)
    for trial in choice["trials"]:
        assert set(trial) == set(gil.KeyFieldsTrial.__annotations__)
        assert set(trial["replay"]) == set(gil.ThresholdReplay.__annotations__)
    assert choice["typed"] is not None
    assert set(choice["typed"]) == set(gil.TypedKeyThresholds.__annotations__)
    with pytest.raises(ValueError, match="several values"):
        gil.select_key_fields(schema, "tags", gil.records([]), typed_longest=1)


def test_the_golden_coarse_choice_and_records_come_back() -> None:
    first = gil.records(lines("records.jsonl")[:CHOSEN])
    schema = gil.Schema.from_dict(golden_form("code", stop_kind="coarse"))
    want = next(
        line
        for line in lines("golden.jsonl")
        if line["kind"] == "coarse" and line["field"] == "code"
    )
    thresholds, records = gil.select_coarse(
        schema, "code", first, 0.8, 10, counting=COUNTED
    )
    for layer in ("key", "memory"):
        same_replay(thresholds[layer], want[layer], f"coarse {layer}")
    bare, none = gil.select_coarse(
        schema, "code", first, 0.8, 10, counting=COUNTED, decisions=False
    )
    assert none is None and bare == thresholds
    expected = [
        line for line in lines("records-out.jsonl") if line["kind"] == "coarse_record"
    ]
    assert records is not None and len(records) == len(expected)
    for i, (record, line) in enumerate(zip(records, expected, strict=True)):
        assert set(record["coarse"]) == set(gil.CoarseLook.__annotations__)
        assert json.loads(
            json.dumps(as_golden(record, "coarse_record")), parse_int=float
        ) == json.loads(json.dumps(line), parse_int=float), (
            f"coarse record line {i + 1}"
        )
    with pytest.raises(ValueError, match="no coarse level"):
        gil.select_coarse(gil.Schema.from_dict(declared_form()), "team", first)


def test_the_golden_key_concentrations_come_back() -> None:
    first = gil.records(lines("records.jsonl")[:CHOSEN])
    keyed = declared_form()
    want = {line["kind"]: line for line in lines("golden.jsonl")[:2]}
    keyed["attributes"][3]["key_fields"] = want["key_fields"]["fields"]
    found = gil.select_key_concentrations(gil.Schema.from_dict(keyed), "team", first)
    assert found == want["key_concentrations"]["concentrations"]
