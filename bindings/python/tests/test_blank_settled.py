"""A blank decided value is refused where it sits, and a time that cannot be read says what it takes."""

from __future__ import annotations

import gil
import pytest

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "decided"},
        ],
    }
)


def line(
    team: str | None, summary: str = "vpn drops", ident: str = "t1"
) -> dict[str, object]:
    values = (
        {"summary": summary} if team is None else {"summary": summary, "team": team}
    )
    settled = {} if team is None else {"team": "Enter"}
    return {
        "id": ident,
        "settled_at": "2026-11-12T10:00:00.0000000+00:00",
        "values": values,
        "settlements": settled,
    }


def test_a_blank_decided_value_names_the_record_and_the_way_out() -> None:
    for blank in ("", "  "):
        with pytest.raises(
            ValueError,
            match=r"record 't1': 'team' is settled as an empty text; leave it out",
        ):
            gil.Decider(SCHEMA, gil.records([line(blank)]))


def test_a_record_that_settled_nothing_and_a_blank_observed_value_read() -> None:
    gil.Decider(
        SCHEMA, gil.records([line(None), line("network", summary="", ident="t2")])
    )


def test_a_keeper_refuses_a_blank_decided_value_too() -> None:
    keeper = gil.Keeper(SCHEMA, target_precision=0.9)
    decider = gil.Decider(keeper.schema)
    with pytest.raises(ValueError, match="'team' is settled as an empty text"):
        keeper.settle(decider, gil.records([line("")]))


def test_a_blank_settlement_is_refused() -> None:
    decider = gil.Decider(SCHEMA)
    record = gil.OpenRecord(SCHEMA, "t3", settled_at=5)
    with pytest.raises(ValueError, match="empty text"):
        record.settle(decider, "team", "Enter", 6, value="")


@pytest.mark.parametrize("found", [None, True, 1.5, [1]])
def test_a_settled_at_that_is_not_a_time_says_what_it_takes(found: object) -> None:
    bad = {"id": "t4", "settled_at": found, "values": {}, "settlements": {}}
    with pytest.raises(
        ValueError, match=r"ticks \(an integer\) or a time yyyy-MM-ddTHH:mm:ss"
    ) as caught:
        gil.records([bad])
    assert "untagged enum" not in str(caught.value)


def test_no_records_count_nothing() -> None:
    assert gil.usage(gil.records([])) == {}
