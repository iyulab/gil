from typing import Dict, Any, List, Optional, Union
from pydantic import BaseModel, Field
import yaml
from pathlib import Path
import os
import re

class NodeConfig(BaseModel):
    type: str
    config: Dict[str, Any] = Field(default_factory=dict)
    inputs: Optional[Dict[str, Any]] = None

class WorkflowConfig(BaseModel):
    name: str
    description: Optional[str] = None
    environment: Dict[str, str] = Field(default_factory=dict)
    nodes: Dict[str, NodeConfig]
    flow: List[Any]

class YamlWorkflowParser:
    def parse_file(self, yaml_path: Union[str, Path]) -> WorkflowConfig:
        yaml_path = Path(yaml_path)
        if not yaml_path.exists():
            raise FileNotFoundError(f"Workflow YAML file not found: {yaml_path}")
        
        with open(yaml_path, 'r', encoding='utf-8') as f:
            yaml_content = f.read()
        
        return self.parse_content(yaml_content)

    def parse_dict(self, config_dict: Dict[str, Any]) -> WorkflowConfig:
        return WorkflowConfig(**config_dict)

    def parse_content(self, yaml_content: str) -> WorkflowConfig:
        # 환경 변수 치환
        processed_content = self._substitute_env_vars(yaml_content)
        
        config_dict = yaml.safe_load(processed_content)
        return WorkflowConfig(**config_dict)

    def _substitute_env_vars(self, content: str) -> str:
        def replace_env(match):
            var_name = match.group(1)
            return os.getenv(var_name, match.group(0)) # 환경변수가 없으면 원본 문자열 반환
        return re.sub(r'\$\{([A-Z_]+)\}', replace_env, content)

    def resolve_references(self, value: Any, context: Dict[str, Any]) -> Any:
        if isinstance(value, str):
            # ${VAR} 형식의 환경 변수 참조 해결
            value = self._substitute_env_vars(value)

            # @node.output 형식의 노드 출력 참조 해결
            match = re.match(r'^@([a-zA-Z0-9_]+)\.([a-zA-Z0-9_]+)$', value)
            if match:
                node_id = match.group(1)
                port_name = match.group(2)
                
                # 여기서는 실제 노드 실행 결과가 아닌, 컨텍스트에서 참조를 해결해야 함
                # 이 부분은 워크플로우 실행 시점에 Executor에서 처리되어야 함
                # 현재는 더미 값 반환 또는 에러 처리
                # TODO: Executor에서 실제 노드 결과 참조 로직 구현
                return f"RESOLVE_ME:{node_id}.{port_name}"
        elif isinstance(value, dict):
            return {k: self.resolve_references(v, context) for k, v in value.items()}
        elif isinstance(value, list):
            return [self.resolve_references(item, context) for item in value]
        return value
