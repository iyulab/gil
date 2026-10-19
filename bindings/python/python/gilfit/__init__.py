"""Python bindings for the gilfit decision core.

Replays settled documents to choose how far each layer's suggestions can be trusted, and hands back the decision
records the choice rests on — one per document asked about, in settlement order.

    import gilfit

    form = gilfit.Form.from_json(text)            # the form and its fields' settings
    documents = gilfit.read_records(path)         # settled documents, one JSON object per line
    thresholds, records = gilfit.select_layers(form, "team", documents, target_precision=0.8)
"""

from __future__ import annotations

import json
from collections.abc import Iterable
from pathlib import Path
from typing import Any, TypedDict, cast

from gilfit._gilfit import Documents, check_form, select_layers_json

__all__ = [
    "DecisionRecord",
    "Documents",
    "Form",
    "KeyLook",
    "KeyState",
    "LayerThresholds",
    "SimilarLook",
    "ThresholdChoice",
    "ThresholdReplay",
    "documents",
    "read_records",
    "select_layers",
]


class ThresholdChoice(TypedDict):
    """A threshold a replay chose, with the precision and promise of the answers down to it."""

    threshold: float
    precision: float
    answer_rate: float
    answered: int
    lookups: int
    answered_weight: float
    effective_answered: float
    similarity_floor: float | None
    promise: float
    promise_answered: float


class KeyState(TypedDict):
    """The state under one key that backed a best value: its lookups, answers and precision."""

    key: str
    lookups: int
    answered: int
    precision: float | None
    answering: bool
    answered_weight: float
    effective_answered: float


class ThresholdReplay(TypedDict):
    """One layer's replay: the chosen and most precise thresholds, and for a key layer the state under each key."""

    chosen: ThresholdChoice | None
    most_precise: ThresholdChoice | None
    lookups: int
    candidates: int
    keys: list[KeyState] | None


class LayerThresholds(TypedDict):
    """The key layer's replay and the memory layer's, chosen on what the key threshold would not have answered."""

    key: ThresholdReplay
    memory: ThresholdReplay


class KeyLook(TypedDict):
    """What the key layer found for one lookup, whatever the threshold."""

    strength: float
    value: str
    correct: bool | None
    key: str
    caught: bool


class SimilarLook(TypedDict):
    """What the similar document vote came to for one lookup, whatever the threshold."""

    score: float
    value: str
    document: str
    correct: bool | None
    nearest: float
    voted: bool


class DecisionRecord(TypedDict):
    """One document asked about on a replay: what each layer made of it and what it was settled as."""

    record: str
    field: str
    order: int | None
    weight: float
    key: KeyLook | None
    similar: SimilarLook | None
    settled: str | None
    settlement: str | None
    probability: float | None


class Form:
    """A form: its name and fields, as JSON ``{"name": ..., "fields": [{"name": ..., "role": "observed" | "judged", ...}]}``.

    A field's optional settings are those of the core's ``Field`` (``key_fields``, ``key_threshold``,
    ``memory_threshold``, ``similar_document_votes``, ``coarse`` …); an unknown setting is an error.
    """

    __slots__ = ("_json",)

    def __init__(self, text: str) -> None:
        check_form(text)
        self._json = text

    @classmethod
    def from_json(cls, text: str) -> Form:
        return cls(text)

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> Form:
        return cls(json.dumps(value, ensure_ascii=False))

    @property
    def json(self) -> str:
        return self._json


def documents(items: Iterable[dict[str, Any]]) -> Documents:
    """Settled documents from dictionaries, in settlement order.

    Each has ``id``, ``settled_at`` (``yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM`` or 100-nanosecond ticks since the Unix
    epoch) and ``values``; ``sets``, ``settlements``, ``probabilities`` and ``arrival`` are optional.
    """
    return Documents.from_json_lines(
        "\n".join(json.dumps(item, ensure_ascii=False) for item in items)
    )


def read_records(path: str | Path) -> Documents:
    """Settled documents from a file of JSON lines, one document per line, in settlement order."""
    return Documents.from_json_lines(Path(path).read_text(encoding="utf-8"))


def select_layers(
    form: Form,
    field: str,
    documents: Documents,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    records: bool = True,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``documents`` and chooses ``field``'s key and memory thresholds.

    Returns the two replays (``{"key": ..., "memory": ...}``, each with its ``chosen`` threshold) and, unless
    ``records`` is false, the decision records: what the key layer and the similar document vote made of each document
    asked about, whatever the thresholds, with the value it was settled as.
    """
    thresholds, lines = select_layers_json(
        form.json,
        field,
        documents,
        target_precision,
        minimum_answered,
        None if counting is None else list(counting),
        records,
    )
    out = (
        None
        if lines is None
        else [cast(DecisionRecord, json.loads(line)) for line in lines.splitlines()]
    )
    return cast(LayerThresholds, json.loads(thresholds)), out
