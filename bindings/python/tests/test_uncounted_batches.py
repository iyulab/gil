"""History loaded in bulk is taken into memory without being counted: ``uncounted_batches`` through Python."""

from __future__ import annotations

import json
from pathlib import Path

import gil
import pytest

CORE = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "core"
IMPORTED = 120


def items() -> list[dict]:
    text = (CORE / "records.jsonl").read_text(encoding="utf-8")
    return [json.loads(line) for line in text.splitlines() if line]


def schema() -> gil.Schema:
    return gil.Schema.from_dict(
        {
            "name": "ticket",
            "attributes": [
                {"name": "channel", "role": "observed"},
                {"name": "product", "role": "observed"},
                {"name": "summary", "role": "observed"},
                {"name": "team", "role": "decided"},
            ],
        }
    )


def with_import() -> list[dict]:
    records = items()[:300]
    return [
        {**r, "batch": "import"} if i < IMPORTED else dict(r)
        for i, r in enumerate(records)
    ]


def lookups(thresholds: dict) -> int:
    return thresholds["key"]["lookups"] + thresholds["memory"]["lookups"]


def test_an_uncounted_batch_is_replayed_but_not_asked_about() -> None:
    records = gil.records(with_import())
    counted, _ = gil.select_layers(schema(), "team", records, decisions=False)
    left_out, _ = gil.select_layers(
        schema(), "team", records, decisions=False, uncounted_batches=["import"]
    )
    assert lookups(left_out) < lookups(counted)
    assert lookups(left_out) > 0
    # An empty list, or one naming another batch, chooses as before.
    for same in ([], ["elsewhere"], None):
        again, _ = gil.select_layers(
            schema(), "team", records, decisions=False, uncounted_batches=same
        )
        assert again == counted


def test_the_other_selections_and_the_keeper_read_it_too() -> None:
    records = gil.records(with_import())
    layers, _ = gil.select_layers(
        schema(), "team", records, decisions=False, uncounted_batches=["import"]
    )

    keeper = gil.Keeper(schema(), uncounted_batches=["import"])
    keeper.settle(gil.Decider(schema()), records)
    assert keeper.rechoose()["chosen"]["team"] == layers

    fields = gil.select_key_fields(
        schema(), "team", records, uncounted_batches=["import"]
    )
    assert isinstance(fields, dict)


def test_a_batch_name_that_is_not_text_is_refused() -> None:
    with pytest.raises(TypeError):
        gil.select_layers(
            schema(),
            "team",
            gil.records(with_import()),
            uncounted_batches=[1],  # type: ignore[list-item]
        )


def test_the_keeper_goes_stale_on_the_records_it_counts() -> None:
    # A tenth of everything settled is 30 records; of the 180 the choice counts, 18.
    keeper = gil.Keeper(schema(), uncounted_batches=["import"])
    decider = gil.Decider(schema())
    keeper.settle(decider, gil.records(with_import()))
    assert (keeper.settled, keeper.counted) == (300, 300 - IMPORTED)
    keeper.rechoose()
    assert keeper.stale() is None

    live = [
        dict(r, id=f"live-{i}", settled_at=f"2027-01-01T00:{i:02d}:00.0000000+00:00")
        for i, r in enumerate(items()[:19])
    ]
    keeper.settle(decider, gil.records(live))
    assert keeper.counted == 300 - IMPORTED + 19
    assert keeper.stale() == {
        "settled": {"chosen": 300 - IMPORTED, "now": 300 - IMPORTED + 19}
    }


def test_without_an_uncounted_batch_every_record_counts() -> None:
    keeper = gil.Keeper(schema())
    keeper.settle(gil.Decider(schema()), gil.records(with_import()))
    assert keeper.counted == keeper.settled == 300
