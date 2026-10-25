"""A keeper's unseen mass (experimental), through Python."""

from __future__ import annotations

import gil

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "component", "role": "observed"},
            {"name": "team", "role": "decided"},
        ],
    }
)


def test_a_keeper_says_how_many_values_were_settled_once() -> None:
    keeper = gil.Keeper(SCHEMA, target_precision=0.5, minimum_answered=5)
    decider = gil.Decider(SCHEMA)
    assert keeper.unseen_mass("team") is None
    teams = ["network", "network", "billing", "auth"]
    keeper.settle(
        decider,
        gil.records(
            {"id": f"t{i}", "settled_at": i + 1, "values": {"component": "c", "team": team}}
            for i, team in enumerate(teams)
        ),
    )
    assert keeper.unseen_mass("team") == {"once": 2, "settled": 4, "mass": 0.5}
    assert keeper.unseen_mass("component") is None
