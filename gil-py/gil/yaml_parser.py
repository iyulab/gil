from typing import Dict, Any, List
from pathlib import Path
import yaml
from pydantic import BaseModel, Field

class NodeConfig(BaseModel):
    type: str
    config: Dict[str, Any] = Field(default_factory=dict)
    inputs: Dict[str, Any] = Field(default_factory=dict)

class WorkflowConfig(BaseModel):
    version: str
    name: str
    nodes: Dict[str, NodeConfig]
    flow: List[Any]
    environment: Dict[str, Any] = Field(default_factory=dict)

class YamlWorkflowParser:
    def parse_file(self, yaml_path: str | Path) -> WorkflowConfig:
        with open(yaml_path, 'r', encoding='utf-8') as f:
            yaml_content = yaml.safe_load(f)
        return self.parse_dict(yaml_content)

    def parse_dict(self, config_dict: Dict[str, Any]) -> WorkflowConfig:
        return WorkflowConfig(**config_dict)

    def resolve_references(self, value: Any, context: Dict[str, Any]) -> Any:
        if isinstance(value, str) and value.startswith("$"):
            # Simple reference resolution for now
            key = value[1:]
            return context.get(key, value) # Return original value if not found
        return value
