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
