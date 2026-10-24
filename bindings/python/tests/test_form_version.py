"""A keeper that starts again under a new edition of the code lists (experimental), through Python."""

from __future__ import annotations

import json

import gil

MINUTE = 60 * 10_000_000

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "component", "role": "observed"},
            {"name": "team", "role": "decided"},
        ],
    }
)


def settled(start: int, stop: int, edition: str | None) -> gil.Records:
    teams = ["network", "billing", "auth"]

    def line(i: int) -> dict:
        team = teams[i % 7 % 3]
        record = {
            "id": f"t{i}",
            "settled_at": i * MINUTE,
            "values": {"component": f"c{i % 7}", "team": f"{team}-2" if edition == "2" else team},
            "settlements": {"team": "Enter"},
        }
        if edition is not None:
            record["form_version"] = edition
        return record

    return gil.records(line(i) for i in range(start, stop))


def test_a_keeper_told_to_restart_keeps_the_new_edition_alone() -> None:
    keeper = gil.Keeper(SCHEMA, target_precision=0.5, minimum_answered=5, restart_on_form_version=True)
    decider = gil.Decider(SCHEMA)
    keeper.settle(decider, settled(0, 200, "1"))
    first = keeper.rechoose()
    assert first["form_version"] == "1"
    assert "team" in first["chosen"]

    keeper.settle(decider, settled(200, 300, "2"))
    keeper.settle(decider, settled(300, 310, "1"))
    assert keeper.form_version == "2"
    assert keeper.settled == 100
    assert keeper.state()["chosen"] == {}
    assert json.loads(keeper.schema.json)["attributes"][1].get("key_threshold") is None
    assert "team" in keeper.rechoose()["chosen"]


def test_without_restarting_every_edition_is_kept() -> None:
    keeper = gil.Keeper(SCHEMA, target_precision=0.5, minimum_answered=5)
    decider = gil.Decider(SCHEMA)
    keeper.settle(decider, settled(0, 100, "1"))
    keeper.settle(decider, settled(100, 150, "2"))
    keeper.settle(decider, settled(150, 160, None))
    assert keeper.settled == 160
    assert keeper.form_version is None
