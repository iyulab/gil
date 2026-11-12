"""A time given to an open record: ticks, the record line's time text, or a datetime with a zone."""

from __future__ import annotations

from datetime import UTC, datetime, timedelta, timezone

import gil
import pytest

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "component", "role": "observed"},
            {"name": "team", "role": "decided", "key_threshold": 0.1},
        ],
    }
)

TEXT = "2026-11-12T10:00:00.0000000+00:00"


def core_ticks(text: str) -> int:
    """The core's reading of a time text: a record that carries it, reopened, and what the snapshot says."""
    saved = gil.OpenRecord(SCHEMA, "x", 0).snapshot()
    saved["settled_at"] = text
    return int(gil.OpenRecord.reopen(SCHEMA, saved).snapshot()["settled_at"])


def test_a_time_text_reads_as_the_core_reads_it() -> None:
    for text in (
        TEXT,
        "2026-11-12T19:00:00.0000001+09:00",
        "2026-11-12T01:30:00.9999999-08:30",
        "1970-01-01T00:00:00.0000000+00:00",
    ):
        assert gil.OpenRecord(SCHEMA, "t", text).snapshot()["settled_at"] == core_ticks(text)


def test_a_datetime_with_a_zone_is_the_same_instant_as_its_text() -> None:
    moment = datetime(2026, 11, 12, 19, 0, 0, 123456, tzinfo=timezone(timedelta(hours=9)))
    text = "2026-11-12T19:00:00.1234560+09:00"
    assert gil.OpenRecord(SCHEMA, "t", moment).snapshot()["settled_at"] == core_ticks(text)


def test_ticks_go_through_unchanged() -> None:
    assert gil.OpenRecord(SCHEMA, "t", 12_345).snapshot()["settled_at"] == 12_345


def test_a_settlement_takes_the_same_kinds_of_time() -> None:
    decider = gil.Decider(SCHEMA, gil.records([]))
    record = gil.OpenRecord(SCHEMA, "t", TEXT)
    record.observe(decider, "component", "vpn")
    record.settle(decider, "team", "Enter", datetime(2026, 11, 12, 10, 1, tzinfo=UTC), value="network")
    assert record.snapshot()["settlements"] == {"team": "Enter"}


@pytest.mark.parametrize(
    "bad",
    [
        datetime(2026, 11, 12, 10, 0, 0),  # no zone: names no instant
        "2026-11-12 10:00:00",  # not the line's time format
        "2026-13-12T10:00:00.0000000+00:00",  # a month out of range
        1.5,
        True,
    ],
)
def test_what_is_not_a_time_is_refused_with_the_reason(bad: object) -> None:
    with pytest.raises((ValueError, TypeError)):
        gil.OpenRecord(SCHEMA, "t", bad)  # type: ignore[arg-type]
