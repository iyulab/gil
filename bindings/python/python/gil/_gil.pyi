"""Type stubs for the gil extension module."""

from typing import final

__all__ = [
    "POOLED_RECENT_ANSWERS",
    "POOLED_RECENT_LOOKUPS",
    "POOLED_STANDARD_ERRORS",
    "RECENT_RECORDS",
    "AskPlanner",
    "Decider",
    "Keeper",
    "OpenRecord",
    "Records",
    "check_schema",
    "schema_with_layers_json",
    "select_coarse_json",
    "select_key_concentrations_json",
    "select_key_fields_json",
    "select_layers_json",
    "tuning_apply_json",
    "tuning_capture_json",
    "tuning_promise_json",
    "tuning_stale_json",
    "usage_json",
]

RECENT_RECORDS: int
"""How many of the latest records replayed a choice's ``promise`` is taken over."""
POOLED_STANDARD_ERRORS: float
"""How many standard errors below the measured precision the pooled rule holds the target at."""
POOLED_RECENT_LOOKUPS: int
"""How many of the attribute's latest lookups the pooled rule chooses on, at the least."""
POOLED_RECENT_ANSWERS: int
"""How many answers the pooled rule reaches back for when its latest lookups hold fewer."""

@final
class Records:
    """Settled records, read once and kept on the Rust side."""

    @staticmethod
    def from_json_lines(text: str) -> Records:
        """Records from JSON lines, one record per line, in settlement order."""

    def __len__(self) -> int: ...

def check_schema(schema_json: str) -> None:
    """Checks a schema's JSON; raises ValueError when it does not read."""

def select_layers_json(
    schema_json: str,
    attribute: str,
    records: Records,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    decisions: bool = True,
    rule: str = "every_band",
    threads: int | None = None,
) -> tuple[str, str | None]:
    """The attribute's key and memory threshold replays as JSON and, when `decisions` is true, the decision records as JSON lines."""

def select_key_fields_json(
    schema_json: str,
    attribute: str,
    records: Records,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    typed_longest: int = 0,
) -> str:
    """The attributes chosen to key the decided attribute, with the key layer's replay for each set tried, as JSON."""

def select_coarse_json(
    schema_json: str,
    attribute: str,
    records: Records,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    decisions: bool = True,
    threads: int | None = None,
) -> tuple[str, str | None]:
    """The attribute's coarse level thresholds as JSON and, when `decisions` is true, the decision records as JSON lines."""

def select_key_concentrations_json(
    schema_json: str, attribute: str, records: Records
) -> str:
    """The concentration chosen for each attribute that keys the decided attribute, as a JSON object."""

@final
class Decider:
    """The memories that answer: the settled key memory and, unless left out, the lexical similar record memory."""

    def __new__(
        cls,
        schema_json: str,
        records: Records | None = None,
        similar_records: bool = True,
        threads: int | None = None,
    ) -> Decider: ...
    def put(self, schema_json: str, records: Records) -> None:
        """Puts `records` (settled records of the schema, in settlement order, later than those it holds) into the memories."""

    def remove(self, schema_json: str, record_id: str) -> None:
        """Takes record `record_id` out of the memories — a record the host deleted; nothing happens when they do not hold it."""

    def answer(
        self,
        schema_json: str,
        id: str,
        attribute: str,
        values_json: str,
        sets_json: str | None = None,
        decisions: bool = False,
    ) -> str:
        """The answer for one open decided attribute of record `id` from the values given, writing nothing to memory, as JSON
        — with `decision`, the lookup's decision record, when `decisions` is true."""

@final
class Keeper:
    """The promise-keeping loop of one schema: the records settled and the replays the thresholds are chosen on."""

    def __new__(
        cls,
        schema_json: str,
        target_precision: float,
        minimum_answered: int,
        counting: list[str] | None = None,
        rule: str = "every_band",
        threads: int | None = None,
    ) -> Keeper: ...
    def settle(self, decider: Decider, records: Records) -> None:
        """Puts `records` into `decider` and into the next choice."""

    def remove(self, decider: Decider, record_id: str) -> None:
        """Takes record `record_id`, which the host deleted, out of `decider` and out of the records."""

    def stale(self) -> str | None:
        """Why the last choice no longer stands, as JSON, or `None` while it does."""

    def settled(self) -> int:
        """How many records were settled, a record settled again counting again."""

    def rechoose(self, decisions: bool = False) -> tuple[str, str | None]:
        """Chooses again; returns the choice as JSON and, when `decisions` is true, the decision records of the records
        replayed this time as JSON lines."""

    def state(self) -> str:
        """The choice as it stands, as JSON."""

    def schema_json(self) -> str:
        """The schema as chosen, as JSON."""

@final
class AskPlanner:
    """Which observed attribute to ask a person for first, so that a decided attribute gets answered. Experimental."""

    def __new__(
        cls, schema_json: str, attribute: str, askable: list[str], budget: float
    ) -> AskPlanner: ...
    def learn(self, decider: Decider, record_json: str) -> None:
        """Learns from a settled record (JSON, as records are read), in settlement order."""

    def advise(self, answer_json: str, values_json: str) -> str:
        """Advice for an open record from the decided attribute's answer and the record's values, as JSON."""

@final
class OpenRecord:
    """One record of a schema while its values arrive; each change returns the answers it causes, as JSON."""

    def __new__(cls, schema_json: str, id: str, settled_at: int) -> OpenRecord: ...
    @staticmethod
    def reopen(schema_json: str, record_json: str) -> OpenRecord:
        """A saved record (JSON, as a snapshot gives it) reopened as it was left."""

    @property
    def id(self) -> str: ...
    def observe(self, decider: Decider, attribute: str, value: str | None) -> str:
        """Sets (or with `None` clears) an observed attribute's value; the answers it changes, as JSON."""

    def settle(
        self,
        decider: Decider,
        attribute: str,
        kind: str,
        now: int,
        value: str | None = None,
        values: list[str] | None = None,
        probability: float | None = None,
    ) -> str:
        """Settles a decided attribute at `now` (ticks) — `kind` by name, with the value, values or probability it needs; the answers it changes, as JSON."""

    def answer(self, decider: Decider, decisions: bool = False) -> str:
        """Answers for every open decided attribute, as JSON — each with ``decision``, its lookup's decision record,
        when ``decisions`` is true."""

    def answer_partial(
        self, decider: Decider, attribute: str, partial: str | None = None
    ) -> str:
        """The answer for one open decided attribute alone — with `partial`, the text typed into it so far — as JSON."""

    def offer_external(self, decider: Decider, attribute: str, ranked_json: str) -> str:
        """The answer for an open decided attribute with an external judgement's ranked values in it (JSON list of `{"value", "probability", "trusted"}`), as JSON."""

    def snapshot(self) -> str:
        """The record as it stands, to save — JSON in the form records are read in."""

def tuning_capture_json(schema_json: str, settled: int, target_precision: float) -> str:
    """The chosen settings of the schema's decided attributes, chosen on `settled` records for `target_precision`, as JSON."""

def tuning_promise_json(tuning_json: str, attribute: str, layers_json: str) -> str:
    """The tuning with what `attribute`'s layer replay (JSON, as `select_layers_json` gives it) promised, as JSON."""

def schema_with_layers_json(
    schema_json: str, attribute: str, layers_json: str, coarse: bool
) -> str:
    """The schema with what the attribute's layer replay chose put on it — on its coarse level when `coarse` — as JSON."""

def tuning_apply_json(tuning_json: str, schema_json: str) -> str:
    """The schema with the tuning's settings put back on its attributes, as JSON."""

def tuning_stale_json(tuning_json: str, schema_json: str, settled: int) -> str | None:
    """Why the tuning no longer stands for the schema with `settled` records, as JSON — `None` while it does."""

def usage_json(decisions_json: str) -> str:
    """The usage counts of the decisions (a JSON list of `{"attribute", "answer", "settled", "settlement", "probability", "promise"}`), by attribute, as JSON."""
