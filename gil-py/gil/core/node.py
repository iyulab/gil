"""
Gil 노드 기본 클래스 (간단한 버전)
"""

from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional
from pydantic import BaseModel, Field, ConfigDict
import uuid
from .port import InputPort, OutputPort
from .connection import Connection
from .context import NodeContext, FlowContext, Context


class Node(BaseModel, ABC):
    """Gil 노드의 기본 클래스"""
    
    node_id: str = Field(default_factory=lambda: str(uuid.uuid4()), description="노드 고유 ID")
    name: str = Field(description="노드 이름")
    node_type: str = Field(description="노드 타입")
    version: str = Field(default="1.0.0", description="노드 버전")
    node_config: Dict[str, Any] = Field(default_factory=dict, description="노드 설정")
    
    # 포트 정의
    input_ports: List[InputPort] = Field(default_factory=list, description="입력 포트들")
    output_ports: List[OutputPort] = Field(default_factory=list, description="출력 포트들")
    
    # 연결 관리
    input_connections: List[Connection] = Field(default_factory=list, description="입력 연결들")
    output_connections: List[Connection] = Field(default_factory=list, description="출력 연결들")
    
    # 실행 상태
    is_running: bool = Field(default=False, description="실행 중 여부")
    last_execution_time: Optional[float] = Field(default=None, description="마지막 실행 시간")
    
    # 컨텍스트 (런타임에 설정)
    node_context: Optional[NodeContext] = Field(default=None, exclude=True, description="노드 컨텍스트")
    flow_context: Optional[FlowContext] = Field(default=None, exclude=True, description="플로우 컨텍스트")
    
    model_config = ConfigDict(arbitrary_types_allowed=True, extra='allow', use_enum_values=True)
    
    def __init__(self, node_id: str, name: Optional[str] = None, node_type: Optional[str] = None, version: str = "1.0.0", node_config: Optional[Dict[str, Any]] = None, **data):
        super().__init__(node_id=node_id, name=name or node_id, node_type=node_type or self.__class__.__name__, version=version, node_config=node_config if node_config is not None else {}, **data)
    
    @abstractmethod
    async def execute(self, data: Dict[str, Any], context: Context) -> Dict[str, Any]:
        """노드 실행 로직을 정의하는 추상 메서드"""
        pass
    
    def add_input_port(self, port: InputPort):
        self.input_ports.append(port)

    def add_output_port(self, port: OutputPort):
        self.output_ports.append(port)
    
    def get_input_port(self, port_name: str) -> Optional[InputPort]:
        """입력 포트 조회"""
        for port in self.input_ports:
            if port.name == port_name:
                return port
        return None
    
    def get_output_port(self, port_name: str) -> Optional[OutputPort]:
        """출력 포트 조회"""
        for port in self.output_ports:
            if port.name == port_name:
                return port
        return None
