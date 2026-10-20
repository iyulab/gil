"""Python bindings for the Gil decision core (published as ``iyulab-gil``).

Replays settled documents to choose how far each layer's suggestions can be trusted, and hands back the decision
records the choice rests on — one per document asked about, in settlement order.

    import gil

    form = gil.Form.from_json(text)            # the form and its fields' settings
    documents = gil.read_records(path)         # settled documents, one JSON object per line
    thresholds, records = gil.select_layers(form, "team", documents, target_precision=0.8)
"""

from __future__ import annotations

import json
from collections.abc import Iterable
from pathlib import Path
from typing import Any, TypedDict, cast

from gil._gil import (
    Documents,
    check_form,
    select_coarse_json,
    select_key_concentrations_json,
    select_key_fields_json,
    select_layers_json,
)

__all__ = [
    "CoarseLook",
    "DecisionRecord",
    "Documents",
    "Form",
    "KeyFieldsChoice",
    "KeyFieldsTrial",
    "KeyLook",
    "KeyState",
    "LayerThresholds",
    "SimilarLook",
    "ThresholdChoice",
    "ThresholdReplay",
    "TypedKeyThresholds",
    "documents",
    "read_records",
    "select_coarse",
    "select_key_concentrations",
    "select_key_fields",
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


class TypedKeyThresholds(TypedDict):
    """The key layer's replays while a person types: one for each length typed, from one character up."""

    by_length: list[ThresholdReplay]


class KeyFieldsTrial(TypedDict):
    """One set of fields tried as a judged field's key fields, and how its key layer replayed."""

    key_fields: list[str]
    replay: ThresholdReplay
    typed: TypedKeyThresholds | None


class KeyFieldsChoice(TypedDict):
    """The fields that should key a judged field, chosen by replaying its key layer.

    ``key_fields`` is ``None`` when no narrower set gained more than every field the judged field rests on — leave the
    field's key fields unset then.
    """

    key_fields: list[str] | None
    chosen: ThresholdReplay
    all_fields: ThresholdReplay
    trials: list[KeyFieldsTrial]
    typed: TypedKeyThresholds | None


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


class CoarseLook(TypedDict):
    """On a coarse replay, what the field's coarse level made of a lookup: the class the keys back and the class the
    similar documents vote for (each as ``value``), and whether the value layers answered at the field's thresholds."""

    value_answered: bool
    keys: KeyLook | None
    vote: SimilarLook | None


class DecisionRecord(TypedDict):
    """One document asked about on a replay: what each layer made of it and what it was settled as."""

    record: str
    field: str
    order: int | None
    weight: float
    key: KeyLook | None
    similar: SimilarLook | None
    coarse: CoarseLook | None
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


def select_key_fields(
    form: Form,
    field: str,
    documents: Documents,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    typed_longest: int = 0,
) -> KeyFieldsChoice:
    """Chooses the fields that should key ``field`` in the settled field memory by replaying its key layer.

    Each field ``field`` rests on is tried alone, then the best are added one at a time while the key layer's gain over
    the target — (precision − target) × answers — grows. A field that happened to settle with one value for a while
    keys every later document that shares it; narrowing the key fields keeps such a field from answering. Set the
    choice as the field's ``key_fields`` before choosing its thresholds with :func:`select_layers`. With
    ``typed_longest`` above zero the typed key thresholds that go with the choice come back too.
    """
    return cast(
        KeyFieldsChoice,
        json.loads(
            select_key_fields_json(
                form.json,
                field,
                documents,
                target_precision,
                minimum_answered,
                None if counting is None else list(counting),
                typed_longest,
            )
        ),
    )


def select_coarse(
    form: Form,
    field: str,
    documents: Documents,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    records: bool = True,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``documents`` and chooses the thresholds of ``field``'s coarse level.

    The coarse level answers with a class (``{"coarse": {"prefix": 2}}`` or a ``classes`` map) where the value layers
    did not answer at the field's thresholds — set the field's ``key_threshold`` and ``memory_threshold`` from
    :func:`select_layers` first. Returns the level's key and similar document replays and, unless ``records`` is false,
    the decision records with what the level made of each lookup (``coarse``).
    """
    thresholds, lines = select_coarse_json(
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


def select_key_concentrations(form: Form, field: str, documents: Documents) -> dict[str, float]:
    """Chooses, for each field that keys ``field``, how readily its keys are believed after only a few settlements.

    A key field whose keys point at one document's value (a reference number, an outlet) is believed after one or two
    settlements; a broad one only after many. Set the result as the field's ``key_concentrations``. Fields whose keys
    were never seen again are left out; a field that takes several values gets none.
    """
    return cast(
        dict[str, float],
        json.loads(select_key_concentrations_json(form.json, field, documents)),
    )
