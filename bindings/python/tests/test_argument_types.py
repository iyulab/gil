"""A wrong kind of argument is named by its kind — never by an attribute of the binding's own internals."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

import gil
import pytest

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


def records() -> gil.Records:
    return gil.records(
        {
            "id": f"t{i}",
            "settled_at": i * MINUTE,
            "values": {"summary": f"drop {i}", "team": "network"},
            "settlements": {"team": "Enter"},
        }
        for i in range(1, 4)
    )


def test_a_decider_is_not_built_from_records() -> None:
    with pytest.raises(TypeError, match=r"expected Schema, got Records"):
        gil.Decider(records())  # type: ignore[arg-type]


def test_a_keeper_is_not_built_from_a_dict() -> None:
    with pytest.raises(TypeError, match=r"expected Schema, got dict"):
        gil.Keeper({"name": "ticket"})  # type: ignore[arg-type]


def test_settle_with_its_arguments_swapped_names_the_decider() -> None:
    keeper = gil.Keeper(SCHEMA)
    decider = gil.Decider(SCHEMA)
    with pytest.raises(TypeError, match=r"expected Decider, got Records"):
        keeper.settle(records(), decider)  # type: ignore[arg-type]
    with pytest.raises(TypeError, match=r"not an instance of 'Records'"):
        keeper.settle(decider, decider)  # type: ignore[arg-type]


@pytest.mark.parametrize(
    "call",
    [
        pytest.param(
            lambda: gil.select_layers("ticket", "team", records()), id="select_layers"
        ),
        pytest.param(
            lambda: gil.select_key_fields("ticket", "team", records()),
            id="select_key_fields",
        ),
        pytest.param(
            lambda: gil.select_coarse("ticket", "team", records()), id="select_coarse"
        ),
        pytest.param(
            lambda: gil.select_key_concentrations("ticket", "team", records()),
            id="select_key_concentrations",
        ),
        pytest.param(
            lambda: gil.Decider(SCHEMA).put("ticket", records()), id="Decider.put"
        ),
        pytest.param(
            lambda: gil.Decider(SCHEMA).remove("ticket", "t1"), id="Decider.remove"
        ),
        pytest.param(lambda: gil.OpenRecord("ticket", "r1", MINUTE), id="OpenRecord"),
        pytest.param(
            lambda: gil.OpenRecord.reopen("ticket", {}), id="OpenRecord.reopen"
        ),
        pytest.param(lambda: gil.capture_tuning("ticket", 3, 0.8), id="capture_tuning"),
        pytest.param(lambda: gil.apply_tuning({}, "ticket"), id="apply_tuning"),
        pytest.param(lambda: gil.tuning_stale({}, "ticket", 3), id="tuning_stale"),
    ],
)
def test_a_schema_argument_that_is_not_a_schema_is_named(
    call: Callable[[], Any],
) -> None:
    with pytest.raises(TypeError, match=r"expected Schema, got str"):
        call()


@pytest.mark.parametrize(
    "call",
    [
        pytest.param(
            lambda d: gil.AskPlanner(SCHEMA, "team", ["summary"], 1.0).learn(d, {}),
            id="AskPlanner.learn",
        ),
        pytest.param(
            lambda d: gil.OpenRecord(SCHEMA, "r1", MINUTE).answer(d),
            id="OpenRecord.answer",
        ),
        pytest.param(
            lambda d: gil.OpenRecord(SCHEMA, "r1", MINUTE).is_first_of_batch(d),
            id="OpenRecord.is_first_of_batch",
        ),
        pytest.param(
            lambda d: gil.Keeper(SCHEMA).settle(d, records()), id="Keeper.settle"
        ),
        pytest.param(lambda d: gil.Keeper(SCHEMA).remove(d, "t1"), id="Keeper.remove"),
    ],
)
def test_a_decider_argument_that_is_not_a_decider_is_named(
    call: Callable[[Any], Any],
) -> None:
    with pytest.raises(TypeError, match=r"expected Decider, got Records"):
        call(records())


def test_the_right_kinds_still_work() -> None:
    keeper = gil.Keeper(SCHEMA)
    decider = gil.Decider(SCHEMA)
    keeper.settle(decider, records())
    assert gil.OpenRecord(SCHEMA, "r1", 4 * MINUTE).answer(decider) is not None
