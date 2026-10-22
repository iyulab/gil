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
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any, Literal, TypedDict, cast

from gil._gil import AskPlanner as _AskPlanner
from gil._gil import Decider as _Decider
from gil._gil import OpenRecord as _OpenRecord
from gil._gil import (
    POOLED_RECENT_ANSWERS,
    POOLED_RECENT_LOOKUPS,
    POOLED_STANDARD_ERRORS,
    RECENT_RECORDS,
    Records,
    check_schema,
    schema_with_layers_json,
    select_coarse_json,
    select_key_concentrations_json,
    select_key_fields_json,
    select_layers_json,
    tuning_apply_json,
    tuning_capture_json,
    tuning_promise_json,
    tuning_stale_json,
    usage_json,
)

__all__ = [
    "POOLED_RECENT_ANSWERS",
    "POOLED_RECENT_LOOKUPS",
    "POOLED_STANDARD_ERRORS",
    "RECENT_RECORDS",
    "Answer",
    "Answered",
    "AskAdvice",
    "AskPlanner",
    "AttributeUsage",
    "BatchCheck",
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
    "TypedLook",
    "apply_tuning",
    "capture_tuning",
    "promise_of",
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


class BatchCheck(TypedDict):
    """What the batches among the latest records do to a promise — see the crate's ``BatchCheck``.

    Thresholds chosen on half of the latest records and counted on the other half, split by batch and split by record;
    ``across_records`` above ``across_batches`` beyond chance means the batches flatter the promise.
    """

    across_batches: float | None
    across_batches_answered: int
    across_records: float | None
    across_records_answered: int
    batches: int
    batch_size: float | None
    correlation: float | None
    design_effect: float | None


class _Layers(TypedDict):
    key: ThresholdReplay
    memory: ThresholdReplay


class LayerThresholds(_Layers, total=False):
    """The key layer's replay and the memory layer's, chosen on what the key threshold would not have answered.

    ``batches`` is there only when some of the latest records name their ``batch``.
    """

    batches: BatchCheck


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


class TypedLook(TypedDict):
    """On a typed replay, the key layer's best value with some of the settled value typed."""

    strength: float
    value: str
    correct: bool


class Answered(TypedDict):
    """What a suggestion answered: the first candidate, when a layer trusted it, and the threshold the layer held it to —
    ``None`` while a person types, when the key layer is held to the threshold for that many characters."""

    layer: str
    value: str
    threshold: float | None


class DecisionRecord(TypedDict):
    """One record asked about on a replay: what each layer made of it and what it was settled as."""

    record: str
    attribute: str
    order: int | None
    weight: float
    key: KeyLook | None
    similar: SimilarLook | None
    coarse: CoarseLook | None
    typed: list[TypedLook | None] | None
    answered: Answered | None
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

    def with_layers(self, attribute: str, layers: LayerThresholds) -> Schema:
        """The schema with what :func:`select_layers` chose put on the decided ``attribute``: the key layer's
        ``threshold`` as ``key_threshold``, the similar record layer's as ``memory_threshold`` with its
        ``similarity_floor`` as ``memory_floor``. A layer with no ``chosen`` threshold is left unset, so it does not
        answer the attribute. Raises ``ValueError`` for an attribute the schema does not have or that is not decided, or
        a result that breaks a schema rule."""
        return Schema(
            schema_with_layers_json(self._json, attribute, json.dumps(layers), False)
        )

    def with_coarse(self, attribute: str, layers: LayerThresholds) -> Schema:
        """As :meth:`with_layers`, for what :func:`select_coarse` chose: the thresholds go on the attribute's
        ``coarse`` level; ``ValueError`` too for an attribute with none."""
        return Schema(
            schema_with_layers_json(self._json, attribute, json.dumps(layers), True)
        )


def records(items: Iterable[dict[str, Any]]) -> Records:
    """Settled records from dictionaries, in settlement order.

    Each has ``id``, ``settled_at`` (``yyyy-MM-ddTHH:mm:ss.fffffff±HH:MM`` or 100-nanosecond ticks since the Unix
    epoch) and ``values``; ``sets``, ``settlements``, ``probabilities``, ``arrival`` and ``batch`` (the batch the
    record was settled in, for ``select_layers`` to count what batches do to its promise) are optional, as are what the
    host saw and timed — ``suggested``, ``sources``, ``promises``, ``settle_ms``, ``open_ms``, ``settled_by`` and
    ``form_version`` (see the line format in ``docs/settled-records.md``).
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
    rule: Literal["every_band", "pooled"] = "every_band",
    threads: int | None = None,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``records`` and chooses ``attribute``'s key and memory thresholds.

    Returns the two replays (``{"key": ..., "memory": ...}``, each with its ``chosen`` threshold) and, unless
    ``decisions`` is false, the decision records: what the key layer and the similar record vote made of each record
    asked about, whatever the thresholds, with the value it was settled as.

    ``rule`` is how the two thresholds are chosen: ``"every_band"`` (each layer on its own, every band of its answers
    at the target) or ``"pooled"`` (the pair answering the most of the attribute's latest lookups while all their
    answers together hold the target at the bottom of their Wilson interval at 1.645 standard errors — see the crate's
    ``ThresholdRule::Pooled``).

    When some of the latest records name their ``batch``, the thresholds carry ``batches`` (``BatchCheck``): the
    precision of thresholds chosen on half of those records and counted on the other half, split by batch and by
    record, with the batches' size, correlation and design effect. It moves no threshold.

    ``threads`` caps the threads a similar-record lookup over many records is split among (``None``: as many as the
    machine offers, up to eight; ``1``: none). The choice is the same whatever it is.
    """
    thresholds, lines = select_layers_json(
        schema.json,
        attribute,
        records,
        target_precision,
        minimum_answered,
        None if counting is None else list(counting),
        decisions,
        rule,
        threads,
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
    threads: int | None = None,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``records`` and chooses the thresholds of ``attribute``'s coarse level.

    The coarse level answers with a class (``{"coarse": {"prefix": 2}}`` or a ``classes`` map) where the value layers
    did not answer at the attribute's thresholds — set the attribute's ``key_threshold`` and ``memory_threshold`` from
    :func:`select_layers` first. Returns the level's key and similar record replays and, unless ``decisions`` is false,
    the decision records with what the level made of each lookup (``coarse``). ``threads`` is as for
    :func:`select_layers`.
    """
    thresholds, lines = select_coarse_json(
        schema.json,
        attribute,
        records,
        target_precision,
        minimum_answered,
        None if counting is None else list(counting),
        decisions,
        threads,
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
    record memory, holding ``records`` (settled records of ``schema``, in settlement order). ``threads`` caps the
    threads one similar-record lookup over a large memory is split among (``None``: as many as the machine offers, up
    to eight; ``1``: none); answers are the same whatever it is."""

    __slots__ = ("_inner",)

    def __init__(
        self,
        schema: Schema,
        records: Records | None = None,
        similar_records: bool = True,
        threads: int | None = None,
    ) -> None:
        self._inner = _Decider(schema.json, records, similar_records, threads)

    def put(self, schema: Schema, records: Records) -> None:
        """Puts ``records`` — settled later than those the memories hold, in settlement order — into the memories."""
        self._inner.put(schema.json, records)

    def remove(self, schema: Schema, record_id: str) -> None:
        """Takes record ``record_id`` out of the memories — a record the host deleted; nothing happens when they do not
        hold it. The key memory is then as if the record had never been put, and the similar record memory never offers
        it again."""
        self._inner.remove(schema.json, record_id)

    def answer(
        self,
        schema: Schema,
        id: str,
        attribute: str,
        values: Mapping[str, str],
        sets: Mapping[str, Iterable[str]] | None = None,
    ) -> Answer:
        """The answer for one open decided attribute of record ``id`` from the ``values`` (and ``sets``) given, writing
        nothing to memory. The saved version of the record, if the memories hold one, is not evidence for itself."""
        return cast(Answer, json.loads(self._answer(schema, id, attribute, values, sets, False)))

    def answer_with_decision(
        self,
        schema: Schema,
        id: str,
        attribute: str,
        values: Mapping[str, str],
        sets: Mapping[str, Iterable[str]] | None = None,
    ) -> tuple[Answer, DecisionRecord]:
        """As :meth:`answer`, with the lookup's decision record — what the key layer and the similar record layer found
        whatever the thresholds, and what was answered — for a host that keeps them to recount its thresholds later."""
        found = json.loads(self._answer(schema, id, attribute, values, sets, True))
        record = found.pop("decision")
        return cast(Answer, found), cast(DecisionRecord, record)

    def _answer(
        self,
        schema: Schema,
        id: str,
        attribute: str,
        values: Mapping[str, str],
        sets: Mapping[str, Iterable[str]] | None,
        decisions: bool,
    ) -> str:
        return self._inner.answer(
            schema.json,
            id,
            attribute,
            json.dumps(dict(values), ensure_ascii=False),
            None if sets is None else json.dumps({k: list(v) for k, v in sets.items()}, ensure_ascii=False),
            decisions,
        )


class AskAdvice(TypedDict):
    """Whether to ask a person for one attribute of an open record: what asking it has gained per character typed in
    records whose answer showed the same coarse class, whether it is worth asking within the budget, and the gain per
    character asking must reach now (``None`` before anything is learned)."""

    attribute: str
    gain: float
    ask: bool
    threshold: float | None


class AskPlanner:
    """Which observed attribute to ask a person for first, so that ``attribute`` (decided, one value) gets answered.
    Experimental, as in the .NET library.

    It learns from settled records, in the order they were settled, what asking each of the ``askable`` observed
    attributes gained per character typed — the answer without the attribute against the answer with it, where no layer
    answered without it — and advises an open record's missing attributes by that gain, marking those worth asking within
    ``budget``, the share (above 0, at most 1) of the typing asking every time would take::

        planner = gil.AskPlanner(schema, "team", ["product", "channel"], budget=0.3)
        for record in settled:                      # in settlement order
            decider.put(schema, gil.records([record]))
            planner.learn(decider, record)
        answer = decider.answer(schema, "t9", "team", values)
        advice = planner.advise(answer, values)     # the open attributes, highest gain first
    """

    __slots__ = ("_inner",)

    def __init__(
        self, schema: Schema, attribute: str, askable: Iterable[str], budget: float
    ) -> None:
        self._inner = _AskPlanner(schema.json, attribute, list(askable), budget)

    def learn(self, decider: Decider, record: dict[str, Any]) -> None:
        """Learns from a settled record. ``decider``'s memories may hold it, but none settled after it."""
        self._inner.learn(decider._inner, json.dumps(record, ensure_ascii=False))

    def advise(self, answer: Answer, values: Mapping[str, str]) -> list[AskAdvice]:
        """Advice for an open record: every askable attribute it has no value for, by gain per character, highest first
        (in the order given where gains tie), each marked whether to ask."""
        return cast(
            list[AskAdvice],
            json.loads(
                self._inner.advise(
                    json.dumps(answer, ensure_ascii=False),
                    json.dumps(dict(values), ensure_ascii=False),
                )
            ),
        )


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

    def answer_with_decisions(self, decider: Decider) -> list[tuple[Answer, DecisionRecord]]:
        """As :meth:`answer`, each with its lookup's decision record — what the key layer and the similar record layer
        found whatever the thresholds, and what was answered — for a host that keeps them to recount its thresholds
        later."""
        found = json.loads(self._inner.answer(decider._inner, True))
        return [(cast(Answer, a), cast(DecisionRecord, a.pop("decision"))) for a in found]

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
    """The chosen settings of ``schema``'s decided attributes as they now stand, chosen when ``settled`` records — the
    number of them, ``len(records)`` — had been settled, for ``target_precision``: the core's ``Tuning``. Add each layer
    replay's promises with :func:`record_promise`."""
    return cast(
        dict[str, Any],
        json.loads(tuning_capture_json(schema.json, settled, target_precision)),
    )


def record_promise(
    tuning: dict[str, Any], attribute: str, layers: LayerThresholds
) -> dict[str, Any]:
    """``tuning`` with what ``attribute``'s chosen key and memory thresholds promised (from :func:`select_layers`)."""
    return cast(
        dict[str, Any],
        json.loads(
            tuning_promise_json(json.dumps(tuning), attribute, json.dumps(layers))
        ),
    )


def promise_of(tuning: dict[str, Any], attribute: str, source: str) -> float | None:
    """What the layer an answer came from promised for ``attribute``: the key layer's promise for an answer whose
    ``source`` is ``"KeyMemory"``, the similar record layer's for ``"SimilarRecord"``; ``None`` for no answer, an
    outside judgement, an attribute the tuning does not name, or a promise it did not record (:func:`record_promise`).
    It reads the value's layers only: a coarse class offered instead (the answer's ``coarse``) names the layer that backs
    the class, and what the value's layer promised is not what the class's does."""
    field = {"KeyMemory": "key_promise", "SimilarRecord": "memory_promise"}.get(source)
    tuned = next(
        (a for a in tuning["attributes"] if a["attribute"] == attribute), None
    )
    return None if field is None or tuned is None else tuned.get(field)


def apply_tuning(tuning: dict[str, Any], schema: Schema) -> Schema:
    """``schema`` with the tuning's chosen settings put back on its attributes of the same names; raises ``ValueError``
    when the result breaks a schema rule — a tuning chosen for another schema may name key attributes this one does not
    have."""
    return Schema(tuning_apply_json(json.dumps(tuning), schema.json))


def tuning_stale(
    tuning: dict[str, Any], schema: Schema, settled: int
) -> dict[str, Any] | str | None:
    """Why the tuning no longer stands for ``schema`` now that ``settled`` records (the number of them) are settled — ``{"version": {"chosen": ...}}``,
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
