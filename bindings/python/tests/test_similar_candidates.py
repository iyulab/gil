"""The similar record layer's runner-up values as candidates (experimental), through Python."""

from __future__ import annotations

import pytest

import gil

MINUTE = 60 * 10_000_000

HISTORY = [
    ("router keeps dropping the connection every evening", "billing"),
    ("router keeps dropping the connection at night", "outage"),
    ("router keeps dropping connection in the evening", "outage"),
    ("the router is dropping the connection every night", "outage"),
    ("invoice shows a charge twice this month", "billing"),
    ("router connection dropped and the invoice was refunded", "refund"),
]
ASKED = "router keeps dropping the connection every evening again"


def schema(candidates: int | None) -> gil.Schema:
    category: dict[str, object] = {
        "name": "category",
        "role": "decided",
        "memory_threshold": 0.0,
    }
    if candidates is not None:
        category["similar_candidates"] = candidates
    return gil.Schema.from_dict(
        {
            "name": "intake",
            "attributes": [{"name": "note", "role": "observed"}, category],
        }
    )


def answer(found: gil.Schema) -> gil.Answer:
    decider = gil.Decider(found)
    decider.put(
        found,
        gil.records(
            {
                "id": f"d{n}",
                "settled_at": n * MINUTE,
                "values": {"note": note, "category": category},
            }
            for n, (note, category) in enumerate(HISTORY, start=1)
        ),
    )
    return decider.answer(found, "d9", "category", {"note": ASKED})


def from_similar(found: gil.Answer) -> list[gil.Candidate]:
    return [c for c in found["candidates"] if c["source"] == "SimilarRecord"]


def test_it_is_off_unless_the_attribute_asks() -> None:
    found = schema(None)
    assert "similar_candidates" not in found.json
    assert len(from_similar(answer(found))) == 1
    assert len(from_similar(answer(schema(1)))) == 1


def test_the_value_the_vote_chose_stays_first_and_the_runners_up_follow_untrusted() -> (
    None
):
    off = answer(schema(None))
    on = answer(schema(3))
    assert on["candidates"][0] == off["candidates"][0]
    assert on["candidates"][0]["trusted"]
    runners_up = on["candidates"][1:]
    assert [c["value"] for c in runners_up] == ["billing", "refund"]
    assert all(not c["trusted"] and c["source"] == "SimilarRecord" for c in runners_up)
    assert all(
        c["evidence"] is not None and "record" in c["evidence"] for c in runners_up
    )
    # A runner-up is scored by its share of the vote: heaviest first, all of them under one.
    shares = [c["score"] for c in runners_up]
    assert shares == sorted(shares, reverse=True)
    assert 0.0 < sum(shares) < 1.0


def test_only_a_decided_attribute_with_a_single_value_takes_it() -> None:
    with pytest.raises(ValueError, match="similar candidates"):
        gil.Schema.from_dict(
            {
                "name": "intake",
                "attributes": [
                    {"name": "note", "role": "observed", "similar_candidates": 3},
                    {"name": "category", "role": "decided"},
                ],
            }
        )
