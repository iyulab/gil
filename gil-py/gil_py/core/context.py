from typing import Dict, Any, Optional, List
from pydantic import BaseModel, Field
import datetime

class NodeContext(BaseModel):
    node_id: str
    variables: Dict[str, Any] = Field(default_factory=dict)
    internal_state: Dict[str, Any] = Field(default_factory=dict)
    metadata: Dict[str, Any] = Field(default_factory=dict)
    errors: List[Dict[str, Any]] = Field(default_factory=list)

    def get_variable(self, key: str, default: Any = None) -> Any:
        return self.variables.get(key, default)

    def set_variable(self, key: str, value: Any):
        self.variables[key] = value

    def get_internal_state(self, key: str, default: Any = None) -> Any:
        return self.internal_state.get(key, default)

    def set_internal_state(self, key: str, value: Any):
        self.internal_state[key] = value

    def update_metadata(self, key: str, value: Any):
        self.metadata[key] = value

    def add_error(self, message: str, error_type: str = "general", details: Optional[Dict[str, Any]] = None):
        error_entry = {
            "timestamp": datetime.datetime.now().isoformat(),
            "message": message,
            "type": error_type,
            "details": details or {}
        }
        self.errors.append(error_entry)

class FlowContext(BaseModel):
    flow_id: str
    variables: Dict[str, Any] = Field(default_factory=dict)
    shared_data: Dict[str, Any] = Field(default_factory=dict)
    metadata: Dict[str, Any] = Field(default_factory=dict)
    completed_nodes: int = 0
    total_nodes: int = 0

    def get_variable(self, key: str, default: Any = None) -> Any:
        return self.variables.get(key, default)

    def set_variable(self, key: str, value: Any):
        self.variables[key] = value

    def get_shared_data(self, key: str, default: Any = None) -> Any:
        return self.shared_data.get(key, default)

    def set_shared_data(self, key: str, value: Any):
        self.shared_data[key] = value

    def update_metadata(self, key: str, value: Any):
        self.metadata[key] = value

    def increment_completed_nodes(self):
        self.completed_nodes += 1

class Context:
    def __init__(self, initial_data: Dict[str, Any] = None):
        self._data = initial_data if initial_data is not None else {}

    def to_dict(self) -> Dict[str, Any]:
        return self._data

    def get(self, key: str, default: Any = None) -> Any:
        return self._data.get(key, default)

    def set(self, key: str, value: Any):
        self._data[key] = value
