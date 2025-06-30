"""
Gil 연결 시스템
"""

from typing import Optional
from pydantic import BaseModel, Field
import uuid


class Connection(BaseModel):
    """노드 간 연결을 나타내는 클래스"""
    
    connection_id: str = Field(default_factory=lambda: str(uuid.uuid4()), description="연결 고유 ID")
    source_node_id: str = Field(description="출력 노드 ID")
    source_port: str = Field(description="출력 포트명")
    target_node_id: str = Field(description="입력 노드 ID")
    target_port: str = Field(description="입력 포트명")
    active: bool = Field(default=True, description="연결 활성화 상태")
    
    def __str__(self) -> str:
        return f"{self.source_node_id}:{self.source_port} -> {self.target_node_id}:{self.target_port}"
