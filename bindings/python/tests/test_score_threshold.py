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


def test_a_json_string_or_a_single_decision_is_refused_with_the_way_forward() -> None:
    import json

    base = decisions(40, 5)
    for wrong in (json.dumps(base), json.dumps(base).encode(), base[0]):
        with pytest.raises(TypeError, match="select_score_threshold_json"):
            gil.select_score_threshold(wrong, 0.9, 30)  # type: ignore[arg-type]
    # the pair that takes a string still does
    assert "chosen" in gil.select_score_threshold_json(json.dumps(base), 0.9, 30)


def test_the_order_the_decisions_arrive_in_does_not_change_the_threshold() -> None:
    base = decisions(600, 6)
    shuffled = list(reversed(base))
    assert gil.select_score_threshold(shuffled, 0.8, 30) == gil.select_score_threshold(
        base, 0.8, 30
    )


def test_records_that_keep_a_judges_score_give_the_decisions_the_threshold_takes() -> (
    None
):
    def line(
        id: str, at: int, kind: str, shown: str, settled: str, **more: object
    ) -> dict[str, object]:
        return {
            "id": id,
            "settled_at": at,
            "values": {"team": settled},
            "settlements": {"team": kind},
            "suggested": {"team": shown},
            "sources": {"team": "Model"},
            "scores": {"team": 0.75},
            **more,
        }

    saved = gil.records(
        [
            line("a", 1, "Accept", "network", "network"),
            line("b", 2, "Correct", "desk", "network"),
            line("c", 3, "Check", "network", "network", probabilities={"team": 0.5}),
            line("d", 4, "Accept", "network", "network", sources={"team": "KeyMemory"}),
        ]
    )
    found = gil.scored_decisions(saved, "team")
    assert [d["id"] for d in found] == ["a", "b", "c"]
    assert [d["right"] for d in found] == [True, False, True]
    assert found[2]["weight"] == 2.0
    assert gil.scored_decisions(saved, "owner") == []

    # They are what select_score_threshold takes.
    replay = gil.select_score_threshold(found, 0.5, 1)
    assert "chosen" in replay


class Scalar:
    """Stands in for a numpy scalar: a zero-dimensional value that hands over the Python value it holds."""

    ndim = 0

    def __init__(self, value: object) -> None:
        self._value = value

    def item(self) -> object:
        return self._value


def unwrapped(base: list[gil.ScoredDecision], wrap: object) -> list[object]:
    """The decisions with every number and flag put behind ``wrap``."""
    return [
        {
            key: wrap(value) if key != "id" else value  # type: ignore[operator]
            for key, value in decision.items()
        }
        for decision in base
    ]


def test_a_value_that_holds_a_python_number_is_read_as_that_number() -> None:
    base = decisions(600, 7)
    held = unwrapped(base, Scalar)
    assert gil.select_score_threshold(
        held,  # type: ignore[arg-type]
        0.8,
        30,
    ) == gil.select_score_threshold(base, 0.8, 30)


def test_numpy_scalars_are_read_as_the_numbers_they_hold() -> None:
    np = pytest.importorskip("numpy")
    base = decisions(600, 8)
    held: list[object] = [
        {
            "id": d["id"],
            "settled_at": np.int64(d["settled_at"]),
            "score": np.float32(d["score"]),
            "right": np.bool_(d["right"]),
        }
        for d in base
    ]
    narrowed: list[gil.ScoredDecision] = [
        {**d, "score": float(np.float32(d["score"]))} for d in base
    ]
    assert gil.select_score_threshold(
        held,  # type: ignore[arg-type]
        0.8,
        30,
    ) == gil.select_score_threshold(narrowed, 0.8, 30)


def test_a_score_that_is_not_finite_is_refused_with_the_decision_it_belongs_to() -> (
    None
):
    for bad in (float("nan"), float("inf"), float("-inf"), Scalar(float("nan"))):
        base = decisions(40, 9)
        base[17]["score"] = bad  # type: ignore[typeddict-item]
        with pytest.raises(ValueError, match=r"c00017.*score.*not finite.*leave score out"):
            gil.select_score_threshold(base, 0.9, 30)


def test_a_value_of_the_wrong_kind_is_refused_naming_the_decision_and_the_field() -> (
    None
):
    wrong: list[tuple[str, object, str]] = [
        ("settled_at", "2026-09-01T09:00:00+00:00", "whole number"),
        ("settled_at", 1.5, "whole number"),
        ("score", True, "a number"),
        ("score", "0.9", "a number"),
        ("right", "False", "True or False"),  # a string must not turn into True
        ("right", 1, "True or False"),
        ("weight", [1.0], "a number"),
    ]
    for key, value, expected in wrong:
        base = decisions(40, 10)
        base[3][key] = value  # type: ignore[literal-required]
        with pytest.raises(TypeError, match=rf"c00003.*{key}.*{expected}"):
            gil.select_score_threshold(base, 0.9, 30)


def test_a_decision_that_is_not_a_dict_is_refused_with_its_position() -> None:
    with pytest.raises(TypeError, match="decision 1 must be a ScoredDecision dict"):
        gil.select_score_threshold(
            [decisions(1, 11)[0], ("c", 1, 0.5, True)],  # type: ignore[list-item]
            0.9,
            1,
        )
