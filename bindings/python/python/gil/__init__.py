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
from typing import Any, Literal, Required, TypedDict, cast

from gil._gil import AskPlanner as _AskPlanner
from gil._gil import Decider as _Decider
from gil._gil import Keeper as _Keeper
from gil._gil import OpenRecord as _OpenRecord
from gil._gil import (
    POOLED_RECENT_ANSWERS,
    POOLED_RECENT_LOOKUPS,
    POOLED_STANDARD_ERRORS,
    RECENT_RECORDS,
    Records,
    check_schema,
    schema_for_first_of_batch_json,
    schema_with_first_of_batch_json,
    schema_with_layers_json,
    scored_decisions_json,
    select_coarse_json,
    select_key_concentrations_json,
    select_key_fields_json,
    select_layers_json,
    select_score_threshold_json,
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
    "Calibration",
    "CalibrationStep",
    "Candidate",
    "CoarseLook",
    "Decider",
    "DecisionRecord",
    "ElementAnswer",
    "ExternalCandidate",
    "Keeper",
    "KeeperChoice",
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
    "ValueTail",
    "apply_tuning",
    "capture_tuning",
    "coarse_promise_of",
    "first_of_batch_promise_of",
    "promise_of",
    "read_records",
    "record_coarse_promise",
    "record_first_of_batch_promise",
    "record_promise",
    "records",
    "scored_decisions",
    "select_coarse",
    "select_key_concentrations",
    "select_key_fields",
    "select_layers",
    "select_score_threshold",
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


class ValueTail(TypedDict, total=False):
    """Experimental. How the chosen thresholds answered the latest records settled as one value — see the crate's
    ``ValueTail``. ``given_instead`` is left out when no other value was given."""

    value: str
    decisions: int
    answered_otherwise: float
    given_instead: str


class CalibrationStep(TypedDict):
    """Experimental. One step of a calibration: from ``score`` up to the next step's, the chance of being right."""

    score: float
    chance: float


class Calibration(TypedDict):
    """Experimental. What the ``"least_cost"`` rule answers with — see the crate's ``Calibration``: each layer's steps by
    rising score, and the chance at or above which the more likely layer answers (``None`` when none paid)."""

    key: list[CalibrationStep]
    memory: list[CalibrationStep]
    threshold: float | None


class LayerThresholds(_Layers, total=False):
    """The key layer's replay and the memory layer's, chosen on what the key threshold would not have answered.

    ``batches`` is there only when some of the latest records name their ``batch``; ``value_tails`` only when asked
    for; ``calibration`` only under the ``"least_cost"`` rule.
    """

    batches: BatchCheck
    value_tails: list[ValueTail]
    calibration: Calibration


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

    def with_first_of_batch(self, attribute: str, layers: LayerThresholds) -> Schema:
        """Experimental. As :meth:`with_layers`, for what a replay of the first records of batches chose: the
        thresholds go on the attribute's ``first_of_batch``."""
        return Schema(
            schema_with_first_of_batch_json(self._json, attribute, json.dumps(layers))
        )

    def for_first_of_batch(self) -> Schema:
        """Experimental. The schema a first record of a batch — one whose batch has no settled record yet — is
        answered with: each attribute's ``first_of_batch`` key and memory thresholds in place of its value layers'. Pass
        it to :meth:`Decider.answer` when :meth:`Decider.is_first_of_batch` says so; :class:`OpenRecord` does this
        itself."""
        return Schema(schema_for_first_of_batch_json(self._json))


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
    rule: Literal["every_band", "pooled", "least_cost"] = "every_band",
    threads: int | None = None,
    value_tails: bool = False,
) -> tuple[LayerThresholds, list[DecisionRecord] | None]:
    """Replays ``records`` and chooses ``attribute``'s key and memory thresholds.

    Returns the two replays (``{"key": ..., "memory": ...}``, each with its ``chosen`` threshold) and, unless
    ``decisions`` is false, the decision records: what the key layer and the similar record vote made of each record
    asked about, whatever the thresholds, with the value it was settled as.

    ``rule`` is how the two thresholds are chosen: ``"every_band"`` (each layer on its own, every band of its answers
    at the target) or ``"pooled"`` (the pair answering the most of the attribute's latest lookups while all their
    answers together hold the target at the bottom of their Wilson interval at 1.645 standard errors — see the crate's
    ``ThresholdRule::Pooled``). Experimental: ``"least_cost"`` reads the target as where answering pays — a wrong answer
    costs ``1 / (1 - target_precision)`` entries — and chooses one chance threshold on each layer's calibrated chance of
    being right, the more likely layer answering; the thresholds carry the ``calibration``, which
    :meth:`Schema.with_layers` puts on the schema so the decider answers by it.

    When some of the latest records name their ``batch``, the thresholds carry ``batches`` (``BatchCheck``): the
    precision of thresholds chosen on half of those records and counted on the other half, split by batch and by
    record, with the batches' size, correlation and design effect. It moves no threshold.

    Experimental: with ``value_tails``, the thresholds carry ``value_tails`` — for each value settled on at least 30 of
    the latest records asked about, how many there were, the share the chosen thresholds answered with another value
    (``answered_otherwise``) and the value given in its place most often. A promise over the attribute is an average
    over its values; a value with a large share is a candidate for one the attribute must not miss. It moves no
    threshold.

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
        value_tails,
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


class ScoredDecision(TypedDict, total=False):
    """One decision a judge outside Gil made about a case a person later settled (experimental).

    ``id`` and ``settled_at`` (ticks, as a record's) order the decisions; ``score`` is the judge's own, higher meaning
    surer, left out or ``None`` when it gave no candidate; ``right`` is whether its best value was the settled one;
    ``weight`` (default 1) is how many decisions this one counts as, one over the draw probability for a checked case.
    """

    id: Required[str]
    settled_at: Required[int]
    score: float | None
    right: bool
    weight: float


def scored_decisions(records: Records, attribute: str) -> list[ScoredDecision]:
    """Experimental. The decisions a judge outside Gil made about ``attribute``, read from settled records.

    A record counts when the attribute was settled with the judge's answer in view (its ``sources`` entry is ``"Model"``),
    the record keeps the judge's ``scores`` entry for it and the value it suggested, and a person accepted it, corrected
    it or, not having seen it, settled the case again (a check). ``right`` is whether the suggested value is the settled
    one; a check counts as one over the probability it was drawn with. Pass the result to
    :func:`select_score_threshold`. A case the judge was asked and gave no candidate for leaves no trace in a record, so
    the answer rate that comes out is among the cases it answered, unless you add those as decisions without a ``score``.
    A record settled again under the same id appears once for each time it was written: pass the records that count.
    """
    return cast(
        list[ScoredDecision], json.loads(scored_decisions_json(records, attribute))
    )


def select_score_threshold(
    decisions: Iterable[ScoredDecision],
    target_precision: float = 0.8,
    minimum_answered: int = 10,
) -> ThresholdReplay:
    """Experimental. Chooses the threshold of a judge outside Gil from its scores and what people settled.

    The same ``chosen`` threshold, answer rate and promise a memory layer gets (:class:`ThresholdReplay`), from nothing
    but ``(score, right)`` in settlement order. Admit the judge's answers as trusted at or above ``chosen["threshold"]``
    and show ``chosen["promise"]`` beside them; ``chosen`` is ``None`` when no threshold stands. Only "higher is surer"
    is assumed of the score. It does not choose how often to check the judge or where to send what it will not answer.

    ``decisions`` is a list of :class:`ScoredDecision`; the order it arrives in does not matter, they are replayed by
    ``settled_at`` and then ``id``. A JSON string goes to :func:`select_score_threshold_json`.
    """
    if isinstance(decisions, str | bytes | bytearray | Mapping):
        raise TypeError(
            "decisions must be a list of ScoredDecision, not a "
            f"{type(decisions).__name__}; pass a JSON string to select_score_threshold_json instead"
        )
    return cast(
        ThresholdReplay,
        json.loads(
            select_score_threshold_json(
                json.dumps(list(decisions)), target_precision, minimum_answered
            )
        ),
    )


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

    def is_first_of_batch(self, batch: str | None, excluding: str | None = None) -> bool:
        """Experimental. Whether a record of ``batch`` now is the first of its batch: the memories hold no settled
        record of the batch but ``excluding`` (the record asked about). A record that names no batch always is."""
        return self._inner.is_first_of_batch(batch, excluding)

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

    def __init__(
        self, schema: Schema, id: str, settled_at: int, batch: str | None = None
    ) -> None:
        """Experimental: ``batch`` names the batch the record is one of — records settled together, several at once
        from one source. When the schema holds thresholds for the first record of a batch (``first_of_batch``), a
        record whose batch has no other settled record in the decider is answered at those; one that names no batch
        always is."""
        self._inner = _OpenRecord(schema.json, id, settled_at, batch)

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

    @property
    def batch(self) -> str | None:
        """Experimental. The batch the record names, if any."""
        return self._inner.batch

    def is_first_of_batch(self, decider: Decider) -> bool:
        """Experimental. Whether the record is answered as the first of its batch now; its answers' promises are then
        :func:`first_of_batch_promise_of`'s."""
        return self._inner.is_first_of_batch(decider._inner)

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
        shown: Answer | None = None,
        promise: float | None = None,
        suggested: str | None = None,
    ) -> list[Answer]:
        """Settles a decided attribute at ``now``. What was in view goes on the record: ``shown``, the decider's answer,
        when it answered — its first candidate and its ``source`` — with ``promise``, the precision promised for its
        layer (:func:`promise_of`); or ``suggested``, a value that was not the decider's."""
        in_view: dict[str, Any] | None = None
        if shown is not None and shown["answered"]:
            in_view = {"value": shown["candidates"][0]["value"], "source": shown["source"]}
            if promise is not None:
                in_view["promise"] = promise
        elif suggested is not None:
            in_view = {"value": suggested}
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
                    None if in_view is None else json.dumps(in_view, ensure_ascii=False),
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


class ElementAnswer(TypedDict):
    """Experimental. An element of an attribute that takes several values, offered by the similar record layer: its
    ``share`` of the vote — the similarity of the voting records that hold it over the similarity of all of them — and the
    ``promise``, the precision promised for one element offered (``None`` before a choice recorded one)."""

    value: str
    share: float
    promise: float | None


class KeeperChoice(TypedDict, total=False):
    """A keeper's choice: the tuning with its promises (the core's ``Tuning``), each attribute's thresholds with how they
    did on the replay, its coarse level's for an attribute that has one, and — from :meth:`Keeper.rechoose` when asked
    for — the decision records of the records replayed that time."""

    tuning: dict[str, Any]
    chosen: dict[str, LayerThresholds]
    coarse: dict[str, LayerThresholds]
    first_of_batch: dict[str, LayerThresholds]
    form_version: str | None
    decisions: list[DecisionRecord]


class Keeper:
    """The promise-keeping loop as one object: settled records go in once, the decider learns them, and the thresholds
    are chosen again when they are stale — on a replay carried on from where the last choice left it, not from the
    start.

    It holds the records and, for each decided attribute of ``schema`` that takes one value, the replay as it stands.
    :meth:`settle` puts records into the decider and into the next choice, :meth:`stale` says when the choice no longer
    stands, and :meth:`rechoose` replays only the records settled since and chooses again — the same thresholds, to the
    bit, as :func:`select_layers` on all the records. An attribute with a coarse level has the level's thresholds chosen
    on the same replay, after its value layers' — as :func:`select_coarse` with the value thresholds just chosen. Answer with the decider and :attr:`schema`; an answer's promise is
    ``promise_of(keeper.tuning, attribute, answer["source"])``. Give it the same decider every time. The choosing
    arguments are :func:`select_layers`'. Nothing is stored outside the process.

    Experimental: with ``first_of_batch``, the first records of batches — whose batch had no settled record when they
    were answered — have the value layers' thresholds chosen and promised apart from the rest, each on its own lookups
    and resting on ``minimum_answered``; the choice's ``first_of_batch`` holds theirs, :attr:`schema` carries them, and
    a first record's promise is ``first_of_batch_promise_of(keeper.tuning, attribute, answer["source"])``.

    Experimental: with ``restart_on_form_version``, a record under a new edition of the code lists (its
    ``form_version`` — new only when a value comes to mean something else) starts the keeper again from nothing: the
    records settled before it leave the decider and the replays, the thresholds chosen on them leave :attr:`schema`,
    and records still arriving under an edition left behind are not taken. A record that names no edition is taken as
    of the current one; the first edition named starts nothing. :attr:`form_version` is the edition kept.

    Experimental: with ``value_tails``, each attribute's thresholds carry ``value_tails`` as :func:`select_layers`'
    do — counted on the records its value thresholds were chosen on.

    Experimental: with ``watch_realized``, :meth:`stale` also says when the answers stopped keeping the promise —
    ``{"realized": {"attribute": ..., "right": ..., "judged": 100}}`` once fewer of an attribute's last 100 answers
    whose rightness came back (accepted, corrected, or checked against the answer in view) were right than the target
    less 0.15. It needs the settlement kinds; records settled with none, or filled by the host, leave it silent.
    Choosing again does not mend a revision of the code lists; naming the new edition (``restart_on_form_version``)
    does.
    """

    __slots__ = ("_inner",)

    def __init__(
        self,
        schema: Schema,
        target_precision: float = 0.8,
        minimum_answered: int = 10,
        counting: Iterable[str] | None = None,
        rule: Literal["every_band", "pooled", "least_cost"] = "every_band",
        threads: int | None = None,
        first_of_batch: bool = False,
        restart_on_form_version: bool = False,
        value_tails: bool = False,
        watch_realized: bool = False,
    ) -> None:
        self._inner = _Keeper(
            schema.json,
            target_precision,
            minimum_answered,
            None if counting is None else list(counting),
            rule,
            threads,
            first_of_batch,
            restart_on_form_version,
            value_tails,
            watch_realized,
        )

    def settle(self, decider: Decider, records: Records) -> None:
        """Puts ``records`` into ``decider`` and into the next choice. Records may come in any order; one settled before
        the last a replay has gone through has the next choice replay from the start."""
        self._inner.settle(decider._inner, records)

    def remove(self, decider: Decider, record_id: str) -> None:
        """Takes record ``record_id``, which the host deleted, out of ``decider`` and out of the records; the next
        choice replays from the start without it."""
        self._inner.remove(decider._inner, record_id)

    def stale(self) -> dict[str, Any] | str | None:
        """Why the last choice no longer stands — ``{"settled": {"chosen": ..., "now": ...}}`` once a tenth more or fewer
        records are settled, and before it, with ``watch_realized``, ``{"realized": ...}`` for the first attribute whose
        answers stopped keeping the promise — or ``None`` while it does."""
        text = self._inner.stale()
        return None if text is None else cast(dict[str, Any] | str, json.loads(text))

    def unseen_mass(self, attribute: str) -> dict[str, Any] | None:
        """Experimental. The share of ``attribute``'s next settled values likely to be ones no record settled yet —
        ``{"once": ..., "settled": ..., "mass": ...}``, the values settled on exactly one record over the records that
        settled one (the Good-Turing estimate), on the last settlement of each and leaving out what the application
        filled. Answers never offer such a value. ``None`` for an attribute the keeper does not choose for, or before a
        record settled it."""
        text = self._inner.unseen_mass(attribute)
        return None if text is None else cast(dict[str, Any], json.loads(text))

    def similar_elements(
        self,
        decider: Decider,
        id: str,
        attribute: str,
        values: Mapping[str, str],
        sets: Mapping[str, Iterable[str]] | None = None,
    ) -> list[ElementAnswer]:
        """Experimental. The elements of ``attribute`` — an attribute that takes several values and sets
        ``"similar_elements": true`` — that the similar record layer offers for record ``id`` from the ``values`` (and
        ``sets``) given, most shared first. The nearest ``similar_votes`` settled records other than ``id`` vote, an element
        is judged on its own by its share of the vote, and it is offered when the share reaches the memory threshold the
        last :meth:`rechoose` found — none when it found none. The ``promise`` is about one element at a time: the share of
        the elements offered that the record settles, not the chance that a whole set is right. What ``sets`` holds for
        ``attribute`` is not evidence. Raises ``ValueError`` for an attribute that is not answered that way."""
        found = self._inner.similar_elements(
            decider._inner,
            id,
            attribute,
            json.dumps(dict(values), ensure_ascii=False),
            None if sets is None else json.dumps({k: list(v) for k, v in sets.items()}, ensure_ascii=False),
        )
        return cast(list[ElementAnswer], json.loads(found))

    @property
    def settled(self) -> int:
        """How many records were settled, a record settled again counting again."""
        return self._inner.settled()

    @property
    def form_version(self) -> str | None:
        """The edition of the code lists the keeper keeps records of — the last one a record named — or ``None`` until
        one does, and always when the keeper does not restart. Experimental."""
        return self._inner.form_version()

    def rechoose(self, decisions: bool = False) -> KeeperChoice:
        """Chooses every attribute's thresholds again on all the records settled — replaying only those settled since
        the last choice where it can — puts them on :attr:`schema` and captures :attr:`tuning` with their promises."""
        text, lines = self._inner.rechoose(decisions)
        choice = cast(KeeperChoice, json.loads(text))
        if lines is not None:
            choice["decisions"] = [
                cast(DecisionRecord, json.loads(line)) for line in lines.splitlines()
            ]
        return choice

    def state(self) -> KeeperChoice:
        """The choice as it stands; ``chosen`` is empty before the first :meth:`rechoose`."""
        return cast(KeeperChoice, json.loads(self._inner.state()))

    @property
    def schema(self) -> Schema:
        """The schema as chosen: the one given, with the thresholds the last :meth:`rechoose` put on it."""
        return Schema(self._inner.schema_json())

    @property
    def tuning(self) -> dict[str, Any]:
        """The last choice as a tuning, with each attribute's promises."""
        return self.state()["tuning"]


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
    the class, and its promise is :func:`coarse_promise_of`'s."""
    field = {"KeyMemory": "key_promise", "SimilarRecord": "memory_promise"}.get(source)
    tuned = next(
        (a for a in tuning["attributes"] if a["attribute"] == attribute), None
    )
    return None if field is None or tuned is None else tuned.get(field)


def record_first_of_batch_promise(
    tuning: dict[str, Any], attribute: str, layers: LayerThresholds
) -> dict[str, Any]:
    """Experimental. ``tuning`` with what ``attribute``'s thresholds for first records of batches promised."""
    return cast(
        dict[str, Any],
        json.loads(
            tuning_promise_json(
                json.dumps(tuning), attribute, json.dumps(layers), first_of_batch=True
            )
        ),
    )


def first_of_batch_promise_of(
    tuning: dict[str, Any], attribute: str, source: str
) -> float | None:
    """Experimental. What the layer an answer came from promised for ``attribute`` when the record was answered as the
    first of its batch (:meth:`OpenRecord.is_first_of_batch`); ``None`` as for :func:`promise_of`, and when no
    thresholds for first records were chosen apart — the answer's promise is then :func:`promise_of`'s."""
    field = {"KeyMemory": "first_key_promise", "SimilarRecord": "first_memory_promise"}.get(
        source
    )
    tuned = next(
        (a for a in tuning["attributes"] if a["attribute"] == attribute), None
    )
    return None if field is None or tuned is None else tuned.get(field)


def record_coarse_promise(
    tuning: dict[str, Any], attribute: str, layers: LayerThresholds
) -> dict[str, Any]:
    """``tuning`` with what ``attribute``'s coarse level's chosen thresholds promised (from :func:`select_coarse`)."""
    return cast(
        dict[str, Any],
        json.loads(
            tuning_promise_json(
                json.dumps(tuning), attribute, json.dumps(layers), coarse=True
            )
        ),
    )


def coarse_promise_of(
    tuning: dict[str, Any], attribute: str, source: str
) -> float | None:
    """What the coarse level promised for a class it offered for ``attribute``, by the layer that backs the class (the
    coarse candidate's ``source``): the level's key promise for ``"KeyMemory"``, its memory promise for
    ``"SimilarRecord"``; ``None`` as for :func:`promise_of` (:func:`record_coarse_promise` records them). A class offered
    as a guess — not trusted — was not promised anything."""
    field = {"KeyMemory": "coarse_key_promise", "SimilarRecord": "coarse_memory_promise"}.get(
        source
    )
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


def usage(records: Records) -> dict[str, AttributeUsage]:
    """The usage counts of the decisions ``records`` settled, by attribute. A record keeps what was in view when it was
    settled — ``suggested``, ``sources`` and ``promises`` — beside how each attribute was settled; an attribute counts as
    answered when its record names the layer the answer came from, or was accepted or corrected, and as turned down when
    the answer was in view and nothing was settled. The realized precision is ``right_weight / judged_weight``; it
    should be read against ``promise_sum / promised``."""
    return cast(dict[str, AttributeUsage], json.loads(usage_json(records)))
