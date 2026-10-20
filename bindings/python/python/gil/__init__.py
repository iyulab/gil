"""Python bindings for the Gil decision core (published as ``iyulab-gil``).

Replays settled records to choose how far each layer's answers can be trusted, and hands back the decision
records the choice rests on — one per record asked about, in settlement order.

    import gil

    schema = gil.Schema.from_json(text)      # the schema and its attributes' settings
    records = gil.read_records(path)         # settled records, one JSON object per line
    thresholds, decisions = gil.select_layers(schema, "team", records, target_precision=0.8)

An open record answers while its values arrive, from a decider whose memories hold the settled records:

    decider = gil.Decider(schema, records)
    record = gil.OpenRecord(schema, "t9", settled_at=now)
    answers = record.observe(decider, "summary", "vpn drops")
    record.settle(decider, "team", "Accept", now, value="network")
    saved = record.snapshot()
"""

from __future__ import annotations

import json
from collections.abc import Iterable
from pathlib import Path
from typing import Any, TypedDict, cast

from gil._gil import Decider as _Decider
from gil._gil import OpenRecord as _OpenRecord
from gil._gil import (
    Records,
    check_schema,
    select_coarse_json,
    select_key_concentrations_json,
    select_key_fields_json,
    select_layers_json,
    tuning_apply_json,
    tuning_capture_json,
    tuning_stale_json,
    usage_json,
)

__all__ = [
    "Answer",
    "AttributeUsage",
    "Candidate",
    "CoarseLook",
    "Decider",
    "DecisionRecord",
    "ExternalCandidate",
    "KeyFieldsChoice",
    "KeyFieldsTrial",
    "KeyLook",
    "KeyState",
    "LayerThresholds",
    "OpenRecord",
    "Records",
    "Schema",
    "SimilarLook",
    "ThresholdChoice",
    "ThresholdReplay",
    "TypedKeyThresholds",
    "apply_tuning",
    "capture_tuning",
    "read_records",
    "record_promise",
    "records",
    "select_coarse",
    "select_key_concentrations",
    "select_key_fields",
    "select_layers",
    "tuning_stale",
    "usage",
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


class Candidate(TypedDict):
    """A value offered for an attribute: its score on its layer's scale, the layer (``source``), what backs it — ``{"key":
    {"attribute", "value"}}``, ``{"record": {"id", "similarity"}}`` or ``None`` — and whether the layer trusts it."""

    value: str
    score: float
    source: str
    evidence: dict[str, Any] | None
    trusted: bool


class Answer(TypedDict):
    """One attribute's answer: its candidates, best first, whether the first is trusted (``answered``), the layer of the
    first, the coarse class offered when no layer answered the value, the attribute's policy, and — when neither memory
    had evidence — the (attribute, value) pairs an external judgement would read."""

    attribute: str
    answered: bool
    candidates: list[Candidate]
    source: str
    coarse: Candidate | None
    policy: str
    needs_external: list[tuple[str, str]] | None


class ExternalCandidate(TypedDict):
    """A value an external judgement (the host's model) ranked: its probability and whether the judgement trusts it."""

    value: str
    probability: float
    trusted: bool


class Decider:
    """The memories that answer: the settled key memory and, unless ``similar_records`` is false, the lexical similar
    record memory, holding ``records`` (settled records of ``schema``, in settlement order)."""

    __slots__ = ("_inner",)

    def __init__(
        self,
        schema: Schema,
        records: Records | None = None,
        similar_records: bool = True,
    ) -> None:
        self._inner = _Decider(schema.json, records, similar_records)


class OpenRecord:
    """One record of a schema while its values arrive. Each change puts the record into the decider's memory again and
    returns the answers it changes. Times are 100-nanosecond ticks since the Unix epoch, UTC.

    ``settle`` takes the settlement kind by name — ``Accept``, ``Correct``, ``Enter``, ``Fill``, ``Restore``, ``Set``,
    ``Check`` (with ``probability``), ``Reject``, ``Revert``, and ``AcceptOther`` / ``CorrectOther`` for a value kept or
    changed over a suggestion that was not the decider's — with the ``value`` (or, for an attribute that takes several,
    ``values``) it settles. A call the core refuses (an attribute that is not open, a value where a set belongs) raises
    ``ValueError``.
    """

    __slots__ = ("_inner",)

    def __init__(self, schema: Schema, id: str, settled_at: int) -> None:
        self._inner = _OpenRecord(schema.json, id, settled_at)

    @classmethod
    def reopen(cls, schema: Schema, saved: dict[str, Any]) -> OpenRecord:
        """A saved record — as :meth:`snapshot` gave it — reopened as it was left."""
        record = cls.__new__(cls)
        record._inner = _OpenRecord.reopen(
            schema.json, json.dumps(saved, ensure_ascii=False)
        )
        return record

    @property
    def id(self) -> str:
        return self._inner.id

    def observe(
        self, decider: Decider, attribute: str, value: str | None
    ) -> list[Answer]:
        """Sets (or with ``None`` clears) an observed attribute's value."""
        return cast(
            list[Answer],
            json.loads(self._inner.observe(decider._inner, attribute, value)),
        )

    def settle(
        self,
        decider: Decider,
        attribute: str,
        kind: str,
        now: int,
        value: str | None = None,
        values: Iterable[str] | None = None,
        probability: float | None = None,
    ) -> list[Answer]:
        """Settles a decided attribute at ``now``."""
        return cast(
            list[Answer],
            json.loads(
                self._inner.settle(
                    decider._inner,
                    attribute,
                    kind,
                    now,
                    value,
                    None if values is None else list(values),
                    probability,
                )
            ),
        )

    def answer(self, decider: Decider) -> list[Answer]:
        """Answers for every open decided attribute."""
        return cast(list[Answer], json.loads(self._inner.answer(decider._inner)))

    def answer_partial(
        self, decider: Decider, attribute: str, partial: str | None = None
    ) -> Answer:
        """The answer for one open decided attribute alone — with ``partial``, the text typed into it so far."""
        return cast(
            Answer,
            json.loads(self._inner.answer_partial(decider._inner, attribute, partial)),
        )

    def offer_external(
        self, decider: Decider, attribute: str, ranked: Iterable[ExternalCandidate]
    ) -> Answer:
        """The answer for an open decided attribute with an external judgement's ranked values in it — what the host's
        model made of the evidence an earlier answer asked for (``needs_external``)."""
        return cast(
            Answer,
            json.loads(
                self._inner.offer_external(
                    decider._inner,
                    attribute,
                    json.dumps(list(ranked), ensure_ascii=False),
                )
            ),
        )

    def snapshot(self) -> dict[str, Any]:
        """The record as it stands, to save: in the form :func:`records` and :meth:`reopen` read."""
        return cast(dict[str, Any], json.loads(self._inner.snapshot()))


def capture_tuning(
    schema: Schema, settled: int, target_precision: float
) -> dict[str, Any]:
    """The chosen settings of ``schema``'s decided attributes as they now stand, chosen on ``settled`` records for
    ``target_precision`` — the core's ``Tuning``. Record each layer replay's promises with :func:`record_promise`."""
    return cast(
        dict[str, Any],
        json.loads(tuning_capture_json(schema.json, settled, target_precision)),
    )


def record_promise(
    tuning: dict[str, Any], attribute: str, layers: LayerThresholds
) -> None:
    """Records in ``tuning`` what ``attribute``'s chosen key and memory thresholds promised (from :func:`select_layers`)."""
    tuned = next((a for a in tuning["attributes"] if a["attribute"] == attribute), None)
    if tuned is None:
        raise ValueError(f"the tuning has no attribute '{attribute}'")
    key, memory = layers["key"]["chosen"], layers["memory"]["chosen"]
    tuned["key_promise"] = None if key is None else key["promise"]
    tuned["memory_promise"] = None if memory is None else memory["promise"]


def apply_tuning(tuning: dict[str, Any], schema: Schema) -> Schema:
    """``schema`` with the tuning's chosen settings put back on its attributes of the same names."""
    return Schema(tuning_apply_json(json.dumps(tuning), schema.json))


def tuning_stale(
    tuning: dict[str, Any], schema: Schema, settled: int
) -> dict[str, Any] | str | None:
    """Why the tuning no longer stands for ``schema`` with ``settled`` records — ``{"version": {"chosen": ...}}``,
    ``"schema"`` or ``{"settled": {"chosen": ..., "now": ...}}`` — or ``None`` while it does."""
    text = tuning_stale_json(json.dumps(tuning), schema.json, settled)
    return None if text is None else cast(dict[str, Any] | str, json.loads(text))


class AttributeUsage(TypedDict):
    """One attribute's usage counts, without content."""

    decisions: int
    answered: int
    accepted: int
    corrected: int
    rejected: int
    filled: int
    checked: int
    checked_right: int
    entered: int
    entered_over_answer: int
    accepted_other: int
    corrected_other: int
    promised: int
    promise_sum: float
    right_weight: float
    judged_weight: float


def usage(decisions: Iterable[dict[str, Any]]) -> dict[str, AttributeUsage]:
    """The usage counts of ``decisions``, by attribute. Each decision is ``{"attribute", "answer", "settled",
    "settlement", "probability", "promise"}`` — the trusted answer shown (``None`` when it abstained), the value settled,
    how (by name), a check's draw probability and the promise the answer was made under. The realized precision is
    ``right_weight / judged_weight``; it should be read against ``promise_sum / promised``."""
    return cast(
        dict[str, AttributeUsage],
        json.loads(usage_json(json.dumps(list(decisions), ensure_ascii=False))),
    )
