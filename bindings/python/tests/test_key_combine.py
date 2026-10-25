"""Key combination (experimental), through Python: an attribute that takes its strongest key alone answers as before
when another attribute repeats one of its keys."""

from __future__ import annotations

import gil

MINUTE = 60 * 10_000_000


def schema(combine: str | None, repeated: bool) -> gil.Schema:
    team: dict[str, object] = {"name": "team", "role": "decided"}
    if combine is not None:
        team["key_combine"] = combine
    observed = ["component", "channel"] + (["module"] if repeated else [])
    return gil.Schema.from_dict(
        {"name": "ticket", "attributes": [*({"name": n, "role": "observed"} for n in observed), team]}
    )


def tickets(repeated: bool) -> gil.Records:
    """Tickets whose team follows the component most of the time; with ``repeated``, a module copying the component."""

    def values(i: int) -> dict[str, str]:
        found = {
            "component": f"c{i % 7}",
            "channel": f"h{i % 3}",
            "team": f"t{i % 3}" if i % 5 == 0 else f"t{i % 7 % 4}",
        }
        if repeated:
            found["module"] = found["component"]
        return found

    return gil.records({"id": f"t{i}", "settled_at": (i + 1) * MINUTE, "values": values(i)} for i in range(200))


def ranked(combine: str | None, repeated: bool) -> list[tuple[str, float]]:
    found = schema(combine, repeated)
    decider = gil.Decider(found, tickets(repeated))
    values = {"component": "c3", "channel": "h1"} | ({"module": "c3"} if repeated else {})
    return [(c["value"], c["score"]) for c in decider.answer(found, "new", "team", values)["candidates"]]


def test_taking_the_strongest_answers_as_before_when_an_attribute_repeats_a_key() -> None:
    alone = ranked("max", repeated=False)
    assert alone
    assert ranked("max", repeated=True) == alone
    assert ranked(None, repeated=True) != ranked(None, repeated=False)
    assert ranked("sum", repeated=False) == ranked(None, repeated=False)
