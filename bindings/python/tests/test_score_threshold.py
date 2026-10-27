"""A judge outside Gil's threshold, chosen from its scores and what people settled (experimental)."""

import random

import gil
import pytest


def decisions(n: int, seed: int) -> list[gil.ScoredDecision]:
    """A judge whose score is the chance it is right."""
    draw = random.Random(seed)
    out: list[gil.ScoredDecision] = []
    for i in range(n):
        score = draw.random()
        out.append(
            {
                "id": f"c{i:05}",
                "settled_at": i,
                "score": score,
                "right": draw.random() < score,
            }
        )
    return out


def test_scores_and_outcomes_give_a_threshold_and_a_promise() -> None:
    replay = gil.select_score_threshold(
        decisions(4_000, 1), target_precision=0.8, minimum_answered=30
    )
    chosen = replay["chosen"]
    assert chosen is not None
    assert 0.7 <= chosen["threshold"] <= 0.9
    assert chosen["precision"] >= 0.8
    assert chosen["promise"] <= chosen["precision"]
    assert chosen["lookups"] == 4_000


def test_a_decision_with_no_score_is_asked_and_not_answered() -> None:
    base = decisions(3_000, 2)
    plain = gil.select_score_threshold(base, 0.8, 30)["chosen"]
    silent: list[gil.ScoredDecision] = [
        {"id": f"s{i}", "settled_at": 10_000 + i} for i in range(1_000)
    ]
    with_silence = gil.select_score_threshold([*base, *silent], 0.8, 30)["chosen"]
    assert plain is not None and with_silence is not None
    assert with_silence["threshold"] == plain["threshold"]
    assert with_silence["lookups"] == 4_000
    assert with_silence["answer_rate"] < plain["answer_rate"]


def test_a_score_that_says_nothing_gets_no_threshold() -> None:
    draw = random.Random(3)
    flat: list[gil.ScoredDecision] = [
        {
            "id": f"c{i}",
            "settled_at": i,
            "score": draw.random(),
            "right": draw.random() < 0.5,
        }
        for i in range(4_000)
    ]
    assert gil.select_score_threshold(flat, 0.9, 30)["chosen"] is None


def test_what_the_core_refuses_raises_value_error() -> None:
    with pytest.raises(ValueError):
        gil.select_score_threshold([{"id": "a", "settled_at": 0, "score": 0.5}], 1.5)
    with pytest.raises(ValueError):
        gil.select_score_threshold(
            [{"id": "a", "settled_at": 0, "score": 0.5, "weight": 0.0}], 0.8, 1
        )
    with pytest.raises(ValueError):
        gil.select_score_threshold([{"id": "a"}], 0.8, 1)  # type: ignore[typeddict-item]
