"""A record's attribute names are not checked against a schema — the documented way to see the ones it does not hold."""

from __future__ import annotations

import json

import gil

MINUTE = 60 * 10_000_000

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "decided", "key_threshold": 0.1},
        ],
    }
)


def lines(decided: str) -> list[dict[str, object]]:
    return [
        {
            "id": f"t{i}",
            "settled_at": i * MINUTE,
            "values": {"summary": f"drop {i}", decided: "network"},
            "settlements": {decided: "Enter"},
        }
        for i in range(1, 6)
    ]


def names_outside(schema: gil.Schema, records: gil.Records) -> set[str]:
    held = {attribute["name"] for attribute in json.loads(schema.json)["attributes"]}
    return set(gil.usage(records)) - held


def test_a_misspelt_name_is_read_by_nothing_and_found_by_comparing_names() -> None:
    records = gil.records(lines("Team"))
    keeper = gil.Keeper(SCHEMA)
    keeper.settle(gil.Decider(SCHEMA), records)  # no error: the names are not checked
    assert names_outside(SCHEMA, records) == {"Team"}


def test_names_that_the_schema_holds_leave_nothing_over() -> None:
    assert names_outside(SCHEMA, gil.records(lines("team"))) == set()
