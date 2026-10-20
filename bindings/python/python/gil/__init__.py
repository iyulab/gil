"""Python bindings for the Gil decision core (published as ``iyulab-gil``).

Replays settled records to choose how far each layer's answers can be trusted, and hands back the decision
records the choice rests on — one per record asked about, in settlement order.

    import gil

    schema = gil.Schema.from_json(text)      # the schema and its attributes' settings
    records = gil.read_records(path)         # settled records, one JSON object per line
    thresholds, decisions = gil.select_layers(schema, "team", records, target_precision=0.8)
"""

from __future__ import annotations

import json
from collections.abc import Iterable
from pathlib import Path
from typing import Any, TypedDict, cast

from gil._gil import (
    Records,
    check_schema,
    select_coarse_json,
    select_key_concentrations_json,
    select_key_fields_json,
    select_layers_json,
)

__all__ = [
    "CoarseLook",
    "DecisionRecord",
    "KeyFieldsChoice",
    "KeyFieldsTrial",
    "KeyLook",
    "KeyState",
    "LayerThresholds",
    "Records",
    "Schema",
    "SimilarLook",
    "ThresholdChoice",
    "ThresholdReplay",
    "TypedKeyThresholds",
    "read_records",
    "records",
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
    """One set of attributes tried as a decided attribute's key attributes, and how its key layer replayed."""

    key_fields: list[str]
    replay: ThresholdReplay
    typed: TypedKeyThresholds | None


class KeyFieldsChoice(TypedDict):
    """The attributes that should key a decided attribute, chosen by replaying its key layer.

    ``key_fields`` is ``None`` when no narrower set gained more than every attribute the decided attribute rests on — leave the
    attribute's key attributes unset then.
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
    """What the similar record vote came to for one lookup, whatever the threshold."""

    score: float
    value: str
    record: str
    correct: bool | None
    nearest: float
    voted: bool


class CoarseLook(TypedDict):
    """On a coarse replay, what the attribute's coarse level made of a lookup: the class the keys back and the class the
    similar records vote for (each as ``value``), and whether the value layers answered at the attribute's thresholds."""

    value_answered: bool
    keys: KeyLook | None
    vote: SimilarLook | None


class DecisionRecord(TypedDict):
    """One record asked about on a replay: what each layer made of it and what it was settled as."""

    record: str
    attribute: str
    order: int | None
    weight: float
    key: KeyLook | None
    similar: SimilarLook | None
    coarse: CoarseLook | None
    settled: str | None
    settlement: str | None
    probability: float | None


class Schema:
    """A schema: its name and attributes, as JSON ``{"name": ..., "attributes": [{"name": ..., "role": "observed" | "decided", ...}]}``.

    An attribute's optional settings are those of the core's ``Attribute`` (``key_fields``, ``key_threshold``,
    ``memory_threshold``, ``similar_votes``, ``coarse`` …); an unknown setting is an error.
    """

    __slots__ = ("_json",)

    def __init__(self, text: str) -> None:
        check_schema(text)
        self._json = text

    @classmethod
    def from_json(cls, text: str) -> Schema:
        return cls(text)

    @classmethod
    def from_dict(cls, value: dict[str, Any]) -> Schema:
        return cls(json.dumps(value, ensure_ascii=False))

    @property
    def json(self) -> str:
        return self._json


def records(items: Iterable[dict[str, Any]]) -> Records:
    """Settled records from dictionaries, in settlement order.

    Each has ``id``, ``settled_at`` (``yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM`` or 100-nanosecond ticks since the Unix
    epoch) and ``values``; ``sets``, ``settlements``, ``probabilities`` and ``arrival`` are optional.
    """
    return Records.from_json_lines(
        "\n".join(json.dumps(item, ensure_ascii=False) for item in items)
    )


def read_records(path: str | Path) -> Records:
    """Settled records from a file of JSON lines, one record per line, in settlement order."""
    return Records.from_json_lines(Path(path).read_text(encoding="utf-8"))


def select_layers(
    schema: Schema,
    attribute: str,
    records: Records,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    decisions: bool = True,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``records`` and chooses ``attribute``'s key and memory thresholds.

    Returns the two replays (``{"key": ..., "memory": ...}``, each with its ``chosen`` threshold) and, unless
    ``decisions`` is false, the decision records: what the key layer and the similar record vote made of each record
    asked about, whatever the thresholds, with the value it was settled as.
    """
    thresholds, lines = select_layers_json(
        schema.json,
        attribute,
        records,
        target_precision,
        minimum_answered,
        None if counting is None else list(counting),
        decisions,
    )
    out = (
        None
        if lines is None
        else [cast(DecisionRecord, json.loads(line)) for line in lines.splitlines()]
    )
    return cast(LayerThresholds, json.loads(thresholds)), out


def select_key_fields(
    schema: Schema,
    attribute: str,
    records: Records,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    typed_longest: int = 0,
) -> KeyFieldsChoice:
    """Chooses the attributes that should key ``attribute`` in the settled key memory by replaying its key layer.

    Each attribute ``attribute`` rests on is tried alone, then the best are added one at a time while the key layer's gain over
    the target — (precision − target) × answers — grows. An attribute that happened to settle with one value for a while
    keys every later record that shares it; narrowing the key attributes keeps such an attribute from answering. Set the
    choice as the attribute's ``key_fields`` before choosing its thresholds with :func:`select_layers`. With
    ``typed_longest`` above zero the typed key thresholds that go with the choice come back too.
    """
    return cast(
        KeyFieldsChoice,
        json.loads(
            select_key_fields_json(
                schema.json,
                attribute,
                records,
                target_precision,
                minimum_answered,
                None if counting is None else list(counting),
                typed_longest,
            )
        ),
    )


def select_coarse(
    schema: Schema,
    attribute: str,
    records: Records,
    target_precision: float = 0.8,
    minimum_answered: int = 10,
    counting: Iterable[str] | None = None,
    decisions: bool = True,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``records`` and chooses the thresholds of ``attribute``'s coarse level.

    The coarse level answers with a class (``{"coarse": {"prefix": 2}}`` or a ``classes`` map) where the value layers
    did not answer at the attribute's thresholds — set the attribute's ``key_threshold`` and ``memory_threshold`` from
    :func:`select_layers` first. Returns the level's key and similar record replays and, unless ``decisions`` is false,
    the decision records with what the level made of each lookup (``coarse``).
    """
    thresholds, lines = select_coarse_json(
        schema.json,
        attribute,
        records,
        target_precision,
        minimum_answered,
        None if counting is None else list(counting),
        decisions,
    )
    out = (
        None
        if lines is None
        else [cast(DecisionRecord, json.loads(line)) for line in lines.splitlines()]
    )
    return cast(LayerThresholds, json.loads(thresholds)), out


def select_key_concentrations(
    schema: Schema, attribute: str, records: Records
) -> dict[str, float]:
    """Chooses, for each attribute that keys ``attribute``, how readily its keys are believed after only a few settlements.

    A key attribute whose keys point at one record's value (a reference number, an outlet) is believed after one or two
    settlements; a broad one only after many. Set the result as the attribute's ``key_concentrations``. Attributes whose keys
    were never seen again are left out; an attribute that takes several values gets none.
    """
    return cast(
        dict[str, float],
        json.loads(select_key_concentrations_json(schema.json, attribute, records)),
    )
