"""A keeper that says when the answers stopped keeping the promise (experimental), through Python."""

from __future__ import annotations

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


def judged(start: int, stop: int) -> gil.Records:
    """Records whose team answer was accepted on the even ones and corrected on the odd."""
    return gil.records(
        {
            "id": f"t{i}",
            "settled_at": (i + 1) * MINUTE,
            "values": {"component": f"c{i % 7}", "team": "network"},
            "settlements": {"team": "Accept" if i % 2 == 0 else "Correct"},
        }
        for i in range(start, stop)
    )


def test_a_keeper_told_to_watch_names_the_attribute_whose_answers_fell_short() -> None:
    for watch, expected in [
        (True, {"realized": {"attribute": "team", "right": 50, "judged": 100}}),
        (False, None),
    ]:
        keeper = gil.Keeper(SCHEMA, target_precision=0.9, minimum_answered=5, watch_realized=watch)
        decider = gil.Decider(SCHEMA)
        keeper.settle(decider, judged(0, 100))
        keeper.rechoose()
        assert keeper.stale() == expected
