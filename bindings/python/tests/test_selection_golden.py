"""The Python binding gives the selections the C ABI wrote to the native fixture, number for number.

``tests/fixtures/core/native/selections.json`` holds calls of the selections and the usage counts, each with its inputs
and the result the C ABI gave — the C ABI's own tests hold the file to the core, and the .NET tests hold it to .NET. This
holds Python to it, so the same records give the same choice from the three bindings.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import gil
import pytest

NATIVE = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "core" / "native"
CHOSEN = 200


def same(value: Any) -> Any:
    """A value as JSON gives it back, with whole numbers read as doubles as the fixture writes them."""
    return json.loads(json.dumps(value), parse_int=float)


def calls() -> list[dict[str, Any]]:
    return json.loads((NATIVE / "selections.json").read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def schema() -> gil.Schema:
    return gil.Schema.from_json((NATIVE / "schema.json").read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def records() -> gil.Records:
    lines = (NATIVE / "records.jsonl").read_text(encoding="utf-8").splitlines()
    return gil.records([json.loads(line) for line in lines[:CHOSEN]])


def options(call: dict[str, Any]) -> dict[str, Any]:
    given = call["options"]
    found: dict[str, Any] = {
        "target_precision": given["target_precision"],
        "minimum_answered": given["minimum_answered"],
    }
    if "counting" in given:
        found["counting"] = given["counting"]
    return found


def test_the_selections_give_what_the_c_abi_gave(
    schema: gil.Schema, records: gil.Records
) -> None:
    seen = set()
    for call in calls():
        kind, attribute = call["call"], call.get("attribute")
        what = f"{kind} {attribute}"
        if kind == "layers":
            asked = options(call)
            if "rule" in call["options"]:
                asked["rule"] = call["options"]["rule"]
            thresholds, decisions = gil.select_layers(
                schema,
                attribute,
                records,
                decisions=call["options"].get("decisions", False),
                **asked,
            )
            assert same(thresholds) == same(call["result"]["thresholds"]), what
            if "decisions" in call["result"]:
                assert same(decisions) == same(call["result"]["decisions"]), what
        elif kind == "coarse":
            thresholds, _ = gil.select_coarse(
                schema, attribute, records, decisions=False, **options(call)
            )
            assert same(thresholds) == same(call["result"]["thresholds"]), what
        elif kind == "key_fields":
            found = gil.select_key_fields(
                schema,
                attribute,
                records,
                typed_longest=call["options"]["typed_longest"],
                **options(call),
            )
            assert same(found) == same(call["result"]), what
        elif kind == "key_concentrations":
            assert same(
                gil.select_key_concentrations(schema, attribute, records)
            ) == same(call["result"]), what
        elif kind == "usage":
            assert same(gil.usage(gil.records(call["records"]))) == same(
                call["result"]
            ), what
        else:
            continue
        seen.add(kind)
    assert seen == {"layers", "coarse", "key_fields", "key_concentrations", "usage"}


def test_a_filled_value_is_neither_learned_nor_asked_by_a_selection(
    schema: gil.Schema,
) -> None:
    """The application fills a value without a person: it stays in the record and no selection learns from or asks
    about it. The golden records with filled copies of the first hundred give the thresholds the records alone give."""
    lines = (NATIVE / "records.jsonl").read_text(encoding="utf-8").splitlines()
    people = [json.loads(line) for line in lines[:CHOSEN]]
    fills = []
    for item in people[:100]:
        filled = json.loads(json.dumps(item))
        filled["id"] = f"fill-{item['id']}"
        filled["settled_at"] = item["settled_at"] + 1
        filled["settlements"] = {**filled.get("settlements", {}), "team": "Fill"}
        fills.append(filled)

    alone, _ = gil.select_layers(
        schema, "team", gil.records(people), 0.8, 10, decisions=False
    )
    mixed, _ = gil.select_layers(
        schema, "team", gil.records(people + fills), 0.8, 10, decisions=False
    )
    assert alone["key"]["chosen"] is not None or alone["memory"]["chosen"] is not None
    assert same(mixed) == same(alone)
