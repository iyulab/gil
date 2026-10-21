"""An open record, a tuning and the usage counts, through Python: the answers, settlements and snapshot are the core's."""

from __future__ import annotations

import gil
import pytest

MINUTE = 60 * 10_000_000

SCHEMA = gil.Schema.from_dict(
    {
        "name": "ticket",
        "attributes": [
            {"name": "component", "role": "observed"},
            {"name": "summary", "role": "observed"},
            {"name": "team", "role": "decided", "key_threshold": 0.1},
        ],
    }
)


def settled() -> gil.Records:
    return gil.records(
        {
            "id": f"t{i}",
            "settled_at": i * MINUTE,
            "values": {"component": "vpn", "summary": f"drop {i}", "team": "network"},
            "settlements": {"team": "Enter"},
        }
        for i in range(1, 4)
    )


def test_an_observed_value_brings_an_answer_with_its_key() -> None:
    decider = gil.Decider(SCHEMA, settled())
    record = gil.OpenRecord(SCHEMA, "t9", 9 * MINUTE)
    assert record.id == "t9"
    (team,) = record.observe(decider, "component", "vpn")
    assert team["attribute"] == "team"
    assert team["policy"] == "suggest"
    first = team["candidates"][0]
    assert (first["value"], first["source"]) == ("network", "KeyMemory")
    assert first["evidence"] == {"key": {"attribute": "component", "value": "vpn"}}
    assert team["answered"] == first["trusted"]
    assert (
        record.answer_partial(decider, "team", "net")["candidates"][0]["value"]
        == "network"
    )
    assert record.answer_partial(decider, "team", "sec")["candidates"] == []


def test_a_settlement_is_kept_in_the_snapshot_and_reopens() -> None:
    decider = gil.Decider(SCHEMA, settled())
    record = gil.OpenRecord(SCHEMA, "t9", 9 * MINUTE)
    record.observe(decider, "component", "vpn")
    after = record.settle(decider, "team", "AcceptOther", 10 * MINUTE, value="security")
    assert after == []  # nothing else rests on the team
    saved = record.snapshot()
    assert saved["values"]["team"] == "security"
    assert saved["settlements"] == {"team": "AcceptOther"}
    assert saved["settled_at"] == 10 * MINUTE
    assert saved["arrival"] == ["component", "team"]

    again = gil.OpenRecord.reopen(SCHEMA, saved)
    assert again.snapshot() == saved
    # The saved record reads back as a settled record, too.
    assert len(gil.records([saved])) == 1


def test_an_external_judgement_comes_back_in_the_answer() -> None:
    decider = gil.Decider(SCHEMA)  # nothing settled: no memory has evidence
    record = gil.OpenRecord(SCHEMA, "t1", MINUTE)
    (team,) = record.observe(decider, "summary", "printer jams")
    assert team["needs_external"] == [["summary", "printer jams"]]
    offered = record.offer_external(
        decider, "team", [{"value": "facilities", "probability": 0.9, "trusted": True}]
    )
    assert offered["candidates"][0]["value"] == "facilities"
    assert offered["source"] == "Model"
    assert offered["answered"]


def test_a_misplaced_record_is_refused_and_the_decider_keeps_working() -> None:
    decider = gil.Decider(SCHEMA, settled())
    misplaced = gil.records(
        [{"id": "bad", "settled_at": 9 * MINUTE, "sets": {"team": ["a", "b"]}}]
    )
    with pytest.raises(ValueError, match="record 'bad': 'team' takes one value"):
        decider.put(SCHEMA, misplaced)
    (team,) = gil.OpenRecord(SCHEMA, "t9", 10 * MINUTE).observe(
        decider, "component", "vpn"
    )
    assert team["candidates"][0]["value"] == "network"


def test_a_call_the_core_refuses_is_a_value_error() -> None:
    decider = gil.Decider(SCHEMA)
    record = gil.OpenRecord(SCHEMA, "t1", MINUTE)
    with pytest.raises(ValueError, match="observed; observe it instead"):
        record.settle(decider, "component", "Enter", MINUTE, value="vpn")
    with pytest.raises(ValueError, match="needs a value"):
        record.settle(decider, "team", "Accept", MINUTE)
    with pytest.raises(ValueError, match="probability"):
        record.settle(decider, "team", "Check", MINUTE, value="network")
    with pytest.raises(ValueError, match="unknown settlement kind"):
        record.settle(decider, "team", "Guess", MINUTE, value="network")
    with pytest.raises(ValueError, match="an id"):
        gil.OpenRecord(SCHEMA, " ", MINUTE)
    record.settle(decider, "team", "Reject", MINUTE)
    with pytest.raises(ValueError, match="not open"):
        record.answer_partial(decider, "team")


def test_a_tuning_captures_applies_and_goes_stale() -> None:
    tuning = gil.capture_tuning(SCHEMA, settled=100, target_precision=0.8)
    (team,) = tuning["attributes"]
    assert (team["attribute"], team["key_threshold"]) == ("team", 0.1)

    thresholds, _ = gil.select_layers(
        SCHEMA, "team", settled(), minimum_answered=1, decisions=False
    )
    promised = gil.record_promise(tuning, "team", thresholds)
    chosen = thresholds["key"]["chosen"]
    assert promised["attributes"][0]["key_promise"] == (
        None if chosen is None else chosen["promise"]
    )
    assert team["key_promise"] is None  # the tuning given is left as it was
    with pytest.raises(ValueError, match="no attribute 'lead'"):
        gil.record_promise(tuning, "lead", thresholds)

    bare = gil.Schema.from_dict(
        {
            "name": "ticket",
            "attributes": [
                {"name": "component", "role": "observed"},
                {"name": "summary", "role": "observed"},
                {"name": "team", "role": "decided"},
            ],
        }
    )
    applied = gil.apply_tuning(tuning, bare)
    assert (
        gil.capture_tuning(applied, 100, 0.8)["attributes"][0]["key_threshold"] == 0.1
    )

    assert gil.tuning_stale(tuning, SCHEMA, 105) is None
    assert gil.tuning_stale(tuning, SCHEMA, 110) == {
        "settled": {"chosen": 100, "now": 110}
    }
    other = gil.Schema.from_dict(
        {"name": "ticket", "attributes": [{"name": "team", "role": "decided"}]}
    )
    assert gil.tuning_stale(tuning, other, 100) == "schema"


def test_usage_counts_each_settlement_by_its_meaning() -> None:
    counts = gil.usage(
        [
            {
                "attribute": "team",
                "answer": "network",
                "settled": "network",
                "settlement": "Accept",
                "promise": 0.9,
            },
            {
                "attribute": "team",
                "answer": "network",
                "settled": "security",
                "settlement": "Correct",
                "promise": 0.9,
            },
            {
                "attribute": "team",
                "answer": None,
                "settled": "facilities",
                "settlement": "CorrectOther",
            },
            {"attribute": "team", "answer": None, "settled": None, "settlement": None},
        ]
    )
    team = counts["team"]
    assert (
        team["decisions"],
        team["answered"],
        team["accepted"],
        team["corrected"],
    ) == (4, 2, 1, 1)
    assert team["corrected_other"] == 1
    assert team["right_weight"] / team["judged_weight"] == 0.5
    assert team["promise_sum"] / team["promised"] == pytest.approx(0.9)


def test_answers_and_counts_carry_exactly_the_typed_keys() -> None:
    decider = gil.Decider(SCHEMA, settled())
    record = gil.OpenRecord(SCHEMA, "t9", 9 * MINUTE)
    (team,) = record.observe(decider, "component", "vpn")
    assert set(team) == set(gil.Answer.__annotations__)
    assert set(team["candidates"][0]) == set(gil.Candidate.__annotations__)
    counts = gil.usage(
        [{"attribute": "team", "answer": None, "settled": None, "settlement": None}]
    )
    assert set(counts["team"]) == set(gil.AttributeUsage.__annotations__)
