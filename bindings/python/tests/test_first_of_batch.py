"""The first record of a batch answered at thresholds of its own (experimental), through Python."""

from __future__ import annotations

import json

import gil

MINUTE = 60 * 10_000_000

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "component", "role": "observed"},
            {
                "name": "team",
                "role": "decided",
                "first_of_batch": {"key_threshold": 0.0},
            },
        ],
    }
)


def settled(n: int, size: int = 1) -> gil.Records:
    teams = ["network", "billing", "auth"]
    return gil.records(
        {
            "id": f"t{i}",
            "settled_at": i * MINUTE,
            "values": {"component": f"c{i % 7}", "team": teams[i % 7 % 3]},
            "settlements": {"team": "Enter"},
            "batch": f"b{i // size}",
        }
        for i in range(n)
    )


def test_an_open_record_is_answered_as_the_first_of_its_batch_until_a_mate_is_settled() -> None:
    decider = gil.Decider(SCHEMA, settled(3))
    assert decider.is_first_of_batch("b9")
    assert not decider.is_first_of_batch("b1")
    assert decider.is_first_of_batch("b1", excluding="t1")

    first = gil.OpenRecord(SCHEMA, "n1", 9 * MINUTE, batch="b9")
    assert first.batch == "b9"
    assert first.is_first_of_batch(decider)
    (team,) = first.observe(decider, "component", "c1")
    assert team["candidates"][0]["trusted"]

    later = gil.OpenRecord(SCHEMA, "n2", 9 * MINUTE, batch="b1")
    assert not later.is_first_of_batch(decider)
    (team,) = later.observe(decider, "component", "c1")
    assert not team["candidates"][0]["trusted"]

    saved = first.snapshot()
    assert saved["batch"] == "b9"
    assert gil.OpenRecord.reopen(SCHEMA, saved).batch == "b9"


def test_the_schema_for_a_first_record_puts_its_thresholds_in_place() -> None:
    view = gil.Schema.from_json(SCHEMA.for_first_of_batch().json)
    team = next(a for a in json.loads(view.json)["attributes"] if a["name"] == "team")
    assert team["key_threshold"] == 0.0


def test_a_keeper_chooses_and_promises_the_first_records_apart() -> None:
    plain = gil.Schema.from_dict(
        {
            "name": "ticket",
            "attributes": [
                {"name": "component", "role": "observed"},
                {"name": "team", "role": "decided"},
            ],
        }
    )
    keeper = gil.Keeper(plain, target_precision=0.5, minimum_answered=5, first_of_batch=True)
    decider = gil.Decider(plain)
    keeper.settle(decider, settled(400, size=4))
    choice = keeper.rechoose()
    first = choice["first_of_batch"]["team"]
    tuned = next(a for a in keeper.tuning["attributes"] if a["attribute"] == "team")
    assert tuned["first_of_batch"]
    chosen = first["key"]["chosen"]
    assert chosen is not None
    assert gil.first_of_batch_promise_of(keeper.tuning, "team", "KeyMemory") == chosen["promise"]
    assert tuned["first_key_threshold"] == chosen["threshold"]

    # The same promise recorded by hand on a captured tuning.
    by_hand = gil.record_first_of_batch_promise(
        gil.capture_tuning(keeper.schema, keeper.settled, 0.5), "team", first
    )
    assert gil.first_of_batch_promise_of(by_hand, "team", "KeyMemory") == chosen["promise"]

    # Off, nothing is chosen apart.
    alike = gil.Keeper(plain, target_precision=0.5, minimum_answered=5)
    alike.settle(gil.Decider(plain), settled(400, size=4))
    assert alike.rechoose()["first_of_batch"] == {}
