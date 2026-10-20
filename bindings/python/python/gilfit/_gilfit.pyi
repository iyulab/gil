"""Type stubs for the gilfit extension module."""

from typing import final

__all__ = [
    "Documents",
    "check_form",
    "select_coarse_json",
    "select_key_concentrations_json",
    "select_key_fields_json",
    "select_layers_json",
]

@final
class Documents:
    """Settled documents, read once and kept on the Rust side."""

    @staticmethod
    def from_json_lines(text: str) -> Documents:
        """Documents from JSON lines, one document per line, in settlement order."""

    def __len__(self) -> int: ...

def check_form(form_json: str) -> None:
    """Checks a form's JSON; raises ValueError when it does not read."""

def select_layers_json(
    form_json: str,
    field: str,
    documents: Documents,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    records: bool = True,
) -> tuple[str, str | None]:
    """The field's key and memory threshold replays as JSON and, when `records` is true, the decision records as JSON lines."""

def select_key_fields_json(
    form_json: str,
    field: str,
    documents: Documents,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    typed_longest: int = 0,
) -> str:
    """The fields chosen to key the judged field, with the key layer's replay for each set tried, as JSON."""

def select_coarse_json(
    form_json: str,
    field: str,
    documents: Documents,
    target_precision: float,
    minimum_answered: int,
    counting: list[str] | None = None,
    records: bool = True,
) -> tuple[str, str | None]:
    """The field's coarse level thresholds as JSON and, when `records` is true, the decision records as JSON lines."""

def select_key_concentrations_json(form_json: str, field: str, documents: Documents) -> str:
    """The concentration chosen for each field that keys the judged field, as a JSON object."""
