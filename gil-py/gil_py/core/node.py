"""
Gil 노드 기본 클래스
"""

from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional
from pydantic import BaseModel, Field
import uuid
import asyncio
from .port import GilPort
from .connection import GilConnection
from .data_types import GilDataType
from .context import NodeContext, FlowContext


class GilNode(BaseModel, ABC):
    """Gil 노드의 기본 클래스"""
    
    node_id: str = Field(default_factory=lambda: str(uuid.uuid4()), description="노드 고유 ID")
    name: str = Field(description="노드 이름")
    node_type: str = Field(description="노드 타입")
    version: str = Field(default="1.0.0", description="노드 버전")
    
    # 포트 정의
    input_ports: List[GilPort] = Field(default_factory=list, description="입력 포트들")
    output_ports: List[GilPort] = Field(default_factory=list, description="출력 포트들")
      # 연결 관리
    input_connections: List[GilConnection] = Field(default_factory=list, description="입력 연결들")
    output_connections: List[GilConnection] = Field(default_factory=list, description="출력 연결들")
    
    # 실행 상태
    is_running: bool = Field(default=False, description="실행 중 여부")
    last_execution_time: Optional[float] = Field(default=None, description="마지막 실행 시간")
    
    # 컨텍스트 (런타임에 설정)
    _node_context: Optional[NodeContext] = Field(default=None, exclude=True, description="노드 컨텍스트")
    _flow_context: Optional[FlowContext] = Field(default=None, exclude=True, description="플로우 컨텍스트")
    
    class Config:
        arbitrary_types_allowed = True
        use_enum_values = True
    
    def __init__(self, **data):
        super().__init__(**data)
        self._setup_ports()
    
    @abstractmethod
    def _setup_ports(self) -> None:
        """포트 설정을 정의하는 추상 메서드"""
        pass
    
    @abstractmethod
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """노드 실행 로직을 정의하는 추상 메서드"""
        pass
    
    def get_input_port(self, port_name: str) -> Optional[GilPort]:
        """입력 포트 조회"""
        for port in self.input_ports:
            if port.name == port_name:
                return port
        return None
    
    def get_output_port(self, port_name: str) -> Optional[GilPort]:
        """출력 포트 조회"""
        for port in self.output_ports:
            if port.name == port_name:
                return port
        return None
    
    def validate_inputs(self, inputs: Dict[str, Any]) -> bool:
        """입력 데이터 유효성 검증"""
        for port in self.input_ports:
            if port.required and port.name not in inputs:
                if port.default_value is None:
                    return False
                inputs[port.name] = port.default_value
            
            if port.name in inputs:
                if not port.validate_data(inputs[port.name]):
                    return False
        return True
    
    async def run(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """노드 실행 래퍼"""
        if not self.validate_inputs(inputs):
            raise ValueError(f"Invalid inputs for node {self.name}")
        
        self.is_running = True
        try:
            result = await self.execute(inputs)
            import time
            self.last_execution_time = time.time()
            return result
        finally:
            self.is_running = False
    
    def set_contexts(self, node_context: NodeContext, flow_context: FlowContext) -> None:
        """컨텍스트 설정"""
        self._node_context = node_context
        self._flow_context = flow_context
    
    def get_context_variable(self, path: str, default: Any = None) -> Any:
        """컨텍스트 변수 조회
        
        Args:
            path: 변수 경로 (예: "flow.variables.user_id", "node.internal_state.cache")
            default: 기본값
        """
        if not path:
            return default
            
        parts = path.split(".", 2)
        if len(parts) < 3:
            return default
        
        context_type, section, key = parts[0], parts[1], parts[2]
        
        if context_type == "flow" and self._flow_context:
            if section == "variables":
                return self._flow_context.get_variable(key, default)
            elif section == "shared_data":
                return self._flow_context.get_shared_data(key, default)
            elif section == "metadata":
                return self._flow_context.metadata.get(key, default)
        
        elif context_type == "node" and self._node_context:
            if section == "variables":
                return self._node_context.get_variable(key, default)
            elif section == "internal_state":
                return self._node_context.get_internal_state(key, default)
            elif section == "metadata":
                return self._node_context.metadata.get(key, default)
        
        return default
    
    def set_context_variable(self, path: str, value: Any) -> None:
        """컨텍스트 변수 설정"""
        if not path:
            return
            
        parts = path.split(".", 2)
        if len(parts) < 3:
            return
        
        context_type, section, key = parts[0], parts[1], parts[2]
        
        if context_type == "flow" and self._flow_context:
            if section == "variables":
                self._flow_context.set_variable(key, value)
            elif section == "shared_data":
                self._flow_context.set_shared_data(key, value)
            elif section == "metadata":
                self._flow_context.update_metadata(key, value)
        
        elif context_type == "node" and self._node_context:
            if section == "variables":
                self._node_context.set_variable(key, value)
            elif section == "internal_state":
                self._node_context.set_internal_state(key, value)
            elif section == "metadata":
                self._node_context.update_metadata(key, value)
    
    def log_error(self, message: str, error_type: str = "general", details: Dict[str, Any] = None) -> None:
        """에러 로깅"""
        if self._node_context:
            self._node_context.add_error(message, error_type, details)
    
    async def execute_with_context(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """컨텍스트와 함께 노드 실행"""
        try:
            # 노드 컨텍스트 초기화
            if self._node_context:
                self._node_context.update_metadata("start_time", asyncio.get_event_loop().time())
            
            # 기본 실행 메서드 호출
            result = await self.execute(inputs)
            
            # 실행 완료 처리
            if self._node_context:
                self._node_context.update_metadata("end_time", asyncio.get_event_loop().time())
                
            if self._flow_context:
                self._flow_context.increment_completed_nodes()
            
            return result
            
        except Exception as e:
            # 에러 로깅
            self.log_error(str(e), "execution_error", {"exception_type": type(e).__name__})
            raise
