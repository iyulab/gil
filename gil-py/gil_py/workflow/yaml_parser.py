"""
YAML 워크플로우 파서
"""

import yaml
from typing import Dict, Any, List, Optional
from pathlib import Path
from pydantic import BaseModel, Field


class NodeConfig(BaseModel):
    """노드 설정"""
    type: str = Field(description="노드 타입")
    config: Dict[str, Any] = Field(default_factory=dict, description="노드 설정")
    inputs: Dict[str, Any] = Field(default_factory=dict, description="입력 값")
    condition: Optional[str] = Field(default=None, description="실행 조건")


class WorkflowConfig(BaseModel):
    """워크플로우 설정"""
    name: str = Field(description="워크플로우 이름")
    description: Optional[str] = Field(default=None, description="워크플로우 설명")
    environment: Dict[str, str] = Field(default_factory=dict, description="환경 변수")
    nodes: Dict[str, NodeConfig] = Field(description="노드 정의")
    flow: List[Any] = Field(description="실행 순서")
    outputs: Dict[str, Any] = Field(default_factory=dict, description="출력 설정")


class YamlWorkflowParser:
    """YAML 워크플로우 파서"""
    
    def __init__(self):
        self.config: Optional[WorkflowConfig] = None
    
    def parse_file(self, yaml_path: str | Path) -> WorkflowConfig:
        """YAML 파일에서 워크플로우 설정을 파싱"""
        yaml_path = Path(yaml_path)
        
        if not yaml_path.exists():
            raise FileNotFoundError(f"YAML 파일을 찾을 수 없습니다: {yaml_path}")
        
        with open(yaml_path, 'r', encoding='utf-8') as f:
            yaml_data = yaml.safe_load(f)
        
        return self.parse_dict(yaml_data)
    
    def parse_dict(self, yaml_data: Dict[str, Any]) -> WorkflowConfig:
        """딕셔너리에서 워크플로우 설정을 파싱"""
        # 노드 설정 파싱
        nodes = {}
        for node_name, node_data in yaml_data.get("nodes", {}).items():
            nodes[node_name] = NodeConfig(**node_data)
        
        # 워크플로우 설정 생성
        config_data = {
            "name": yaml_data.get("name", "Unnamed Workflow"),
            "description": yaml_data.get("description"),
            "environment": yaml_data.get("environment", {}),
            "nodes": nodes,
            "flow": yaml_data.get("flow", []),
            "outputs": yaml_data.get("outputs", {})
        }
        
        self.config = WorkflowConfig(**config_data)
        return self.config
    
    def resolve_references(self, value: Any, context: Dict[str, Any]) -> Any:
        """참조 문자열 해결 (@node_name.output 형태)"""
        if isinstance(value, str):
            if value.startswith("@"):
                # @node_name.output 형태의 참조 해결
                ref_path = value[1:]  # @ 제거
                return self._resolve_path(ref_path, context)
            elif value.startswith("${") and value.endswith("}"):
                # ${env_var} 형태의 환경변수 해결
                env_var = value[2:-1]
                return self._resolve_env_var(env_var, context)
        
        return value
    
    def _resolve_path(self, path: str, context: Dict[str, Any]) -> Any:
        """경로 기반 참조 해결"""
        parts = path.split(".")
        current = context        
        for part in parts:
            if isinstance(current, dict):
                current = current.get(part)
            elif isinstance(current, list) and part.isdigit():
                idx = int(part)
                current = current[idx] if idx < len(current) else None
            else:
                return None
        
        return current
    
    def _resolve_env_var(self, env_var: str, context: Dict[str, Any]) -> str:
        """환경변수 및 입력 변수 해결"""
        import os
        
        # 기본값 처리 (| 구분자 사용)
        default_value = None
        if "|" in env_var:
            env_var, default_value = env_var.split("|", 1)
            env_var = env_var.strip()
            default_value = default_value.strip()
        
        # input.* 형태의 입력 변수 처리
        if env_var.startswith("input."):
            input_key = env_var[6:]  # "input." 제거
            input_data = context.get("input", {})
            if input_key in input_data:
                return str(input_data[input_key])
            elif default_value is not None:
                return default_value
            else:
                return f"${{{env_var}}}"
        
        # timestamp 처리
        if env_var == "timestamp":
            from datetime import datetime
            return datetime.now().isoformat()
        
        # 환경변수 또는 입력 컨텍스트에서 값 찾기
        if env_var in context.get("environment", {}):
            return context["environment"][env_var]
        
        # 시스템 환경변수에서 찾기
        value = os.getenv(env_var)
        if value is not None:
            return value
        
        # 기본값이 있으면 사용
        if default_value is not None:
            return default_value
        
        return f"${{{env_var}}}"  # 기본값은 원래 문자열
