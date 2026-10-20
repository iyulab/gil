"""Type stubs for the gil extension module."""

from typing import final

__all__ = [
    "Records",
    "check_schema",
    "select_coarse_json",
    "select_key_concentrations_json",
    "select_key_fields_json",
    "select_layers_json",
]

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
) -> tuple[str, str | None]:
    """The attribute's coarse level thresholds as JSON and, when `decisions` is true, the decision records as JSON lines."""

def select_key_concentrations_json(schema_json: str, attribute: str, records: Records) -> str:
    """The concentration chosen for each attribute that keys the decided attribute, as a JSON object."""
