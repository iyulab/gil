"""The similar record layer of an attribute that takes several values (experimental), through Python."""

from __future__ import annotations

import pytest

import gil

MINUTE = 60 * 10_000_000


def schema(elements: bool) -> gil.Schema:
    tags: dict[str, object] = {"name": "tags", "role": "decided", "multiple": True}
    if elements:
        tags["similar_elements"] = True
    return gil.Schema.from_dict(
        {"name": "desk", "attributes": [{"name": "text", "role": "observed"}, tags]}
    )


def desk() -> gil.Records:
    """Texts that name their topic among noise words; a topic's two tags are always given."""
    return gil.records(
        {
            "id": f"r{i:03}",
            "settled_at": (i + 1) * MINUTE,
            "values": {"text": f"topic{i % 4}x w{(i * 7919) % 97} topic{i % 4}y"},
            "sets": {"tags": [f"core{i % 4}a", f"core{i % 4}b"]},
        }
        for i in range(240)
    )


def test_a_keeper_offers_the_elements_the_similar_records_agree_on_with_the_promise() -> None:
    found = schema(True)
    keeper = gil.Keeper(found, target_precision=0.9, minimum_answered=10)
    decider = gil.Decider(found)
    keeper.settle(decider, desk())
    values = {"text": "topic2x w5 topic2y"}
    assert keeper.similar_elements(decider, "asking", "tags", values) == []

    keeper.rechoose()
    tuned = keeper.tuning["attributes"][0]
    assert tuned["memory_threshold"] is not None
    offered = keeper.similar_elements(decider, "asking", "tags", values, {"tags": ["core0a"]})
    assert [e["value"] for e in offered] == ["core2a", "core2b"]
    assert all(e["share"] > 0.9 and e["promise"] == tuned["memory_promise"] for e in offered)


def test_it_is_off_unless_the_attribute_asks() -> None:
    found = schema(False)
    assert "similar_elements" not in found.json
    keeper = gil.Keeper(found, target_precision=0.9, minimum_answered=10)
    decider = gil.Decider(found)
    keeper.settle(decider, desk())
    assert keeper.rechoose()["chosen"] == {}
    with pytest.raises(ValueError, match="not answered by similar elements"):
        keeper.similar_elements(decider, "asking", "tags", {"text": "topic2x"})
