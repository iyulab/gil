"""A keeper that goes on from a checkpoint chooses what a keeper that replays everything chooses, through Python.

A record settled out of order, taken out, or settled again must leave the keeper's choice (the thresholds and their
promises, ``Keeper.state()``) exactly what a new keeper chooses from the records that are left, in the order they came.
"""

from __future__ import annotations

import json
import random
from typing import Any

import gil
import pytest

DEFINITION = {
    "name": "ticket",
    "attributes": [
        {"name": "product", "role": "observed"},
        {"name": "summary", "role": "observed"},
        {"name": "team", "role": "decided", "coarse": {"prefix": 2}},
        {"name": "queue", "role": "decided"},
    ],
}
PRODUCTS = {
    "vpn": "net-a",
    "mail": "net-b",
    "crm": "app-x",
    "erp": "app-y",
    "wiki": "doc-a",
    "chat": "net-c",
}
WORDS = [
    "drops",
    "slow",
    "login",
    "error",
    "export",
    "sync",
    "timeout",
    "report",
    "access",
    "reset",
    "fails",
    "hourly",
]
KINDS = ["Enter", "Accept", "Correct"]
KEEPS: list[dict[str, Any]] = [
    {"target_precision": 0.8, "minimum_answered": 10},
    {
        "target_precision": 0.75,
        "minimum_answered": 20,
        "rule": "pooled",
        "value_tails": True,
    },
]
OPERATIONS = ("late", "tie", "many", "remove", "again", "inorder", "back")


def row(rng: random.Random, record_id: str, settled_at: int) -> dict[str, Any]:
    product = rng.choice(list(PRODUCTS))
    words = rng.sample(WORDS, 3)
    team = (
        PRODUCTS[product]
        if rng.random() < 0.85
        else rng.choice(list(PRODUCTS.values()))
    )
    queue = "q-" + (words[0] if rng.random() < 0.8 else rng.choice(WORDS))
    return {
        "id": record_id,
        "settled_at": settled_at,
        "values": {
            "product": product,
            "summary": " ".join(words),
            "team": team,
            "queue": queue,
        },
        "settlements": {"team": rng.choice(KINDS), "queue": rng.choice(KINDS)},
    }


def chosen_from_scratch(
    keep: dict[str, Any], arrived: list[dict[str, Any]], removed: set[str]
) -> dict[str, Any]:
    keeper = gil.Keeper(gil.Schema.from_dict(DEFINITION), **keep)
    keeper.settle(
        gil.Decider(keeper.schema),
        gil.records([r for r in arrived if r["id"] not in removed]),
    )
    keeper.rechoose()
    return json.loads(json.dumps(keeper.state(), sort_keys=True))


def drive(
    seed: int, keep: dict[str, Any], leave_out: str | None = None
) -> tuple[dict[str, Any], dict[str, Any], int]:
    """Runs one random schedule; returns the keeper's choice after its last rechoose and the scratch choice of the same
    records, plus how many times they were compared along the way (each compared equal)."""
    rng = random.Random(seed)
    keeper = gil.Keeper(gil.Schema.from_dict(DEFINITION), **keep)
    decider = gil.Decider(keeper.schema)
    arrived: list[dict[str, Any]] = []
    removed: set[str] = set()
    serial = clock = 0

    def settle(rows: list[dict[str, Any]]) -> None:
        arrived.extend(rows)
        keeper.settle(decider, gil.records(rows))

    def fresh() -> str:
        nonlocal serial
        serial += 1
        return f"r{serial}"

    for _ in range(rng.randint(2, 4)):
        batch = []
        for _ in range(rng.randint(60, 140)):
            clock += rng.choice([1, 1, 2])
            batch.append(row(rng, fresh(), clock))
        settle(batch)
        keeper.rechoose()

    compared = 0
    for _ in range(rng.randint(4, 8)):
        for _ in range(rng.randint(1, 3)):
            live = [r for r in arrived if r["id"] not in removed]
            op = rng.choice(OPERATIONS)
            if op == "inorder":
                batch = []
                for _ in range(rng.randint(1, 30)):
                    clock += 1
                    batch.append(row(rng, fresh(), clock))
                settle(batch)
            elif op == "late":
                at = rng.choice(
                    [
                        live[0]["settled_at"] - 1,
                        rng.choice(live)["settled_at"],
                        clock - 1,
                    ]
                )
                settle([row(rng, fresh(), max(0, at))])
            elif op == "tie":
                settle([row(rng, fresh(), rng.choice(live)["settled_at"])])
            elif op == "many":
                settle(
                    [
                        row(rng, fresh(), rng.choice(live)["settled_at"])
                        for _ in range(rng.randint(2, 10))
                    ]
                )
            elif op == "remove" and len(live) > 50:
                victim = rng.choice(live)["id"]
                keeper.remove(decider, victim)
                removed.add(victim)
            elif op == "again":
                victim = rng.choice(live)
                at = rng.choice(
                    [victim["settled_at"], clock + 1, victim["settled_at"] - 1]
                )
                clock = max(clock, at)
                settle([row(rng, victim["id"], at)])
            elif op == "back" and len(live) > 50:
                # taken out, and the same id settled again as a new case
                victim = rng.choice(live)["id"]
                keeper.remove(decider, victim)
                clock += 1
                arrived[:] = [r for r in arrived if r["id"] != victim]
                settle([row(rng, victim, rng.choice([clock, clock - 5]))])
        keeper.rechoose()
        expected = chosen_from_scratch(keep, arrived, removed)
        actual = json.loads(json.dumps(keeper.state(), sort_keys=True))
        assert actual == expected, (
            f"seed {seed} diverged from a keeper that replays everything"
        )
        compared += 1
    if (
        leave_out is not None
    ):  # the negative control: the same schedule, one record fewer on the scratch side
        removed.add(
            leave_out
            if leave_out != "any"
            else next(r["id"] for r in arrived if r["id"] not in removed)
        )
    return actual, chosen_from_scratch(keep, arrived, removed), compared


@pytest.mark.parametrize("keep", KEEPS, ids=["every_band", "pooled_tails"])
def test_a_late_or_removed_or_repeated_record_leaves_the_choice_of_a_keeper_that_replays_everything(
    keep: dict[str, Any],
) -> None:
    compared = 0
    for seed in range(6):
        actual, expected, count = drive(seed, keep)
        assert actual == expected
        compared += count
    assert compared >= 20  # the schedules did reach the comparisons


def test_the_comparison_notices_a_record_that_the_keeper_does_not_have() -> None:
    actual, other, _ = drive(0, KEEPS[0], leave_out="any")
    assert actual != other
