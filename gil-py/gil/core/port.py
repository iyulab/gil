"""
Gil 포트 시스템
"""

from typing import Any, Optional
from pydantic import BaseModel, Field
from .data_types import DataType


class Port(BaseModel):
    """Gil 노드의 입출력 포트"""
    
    name: str = Field(description="포트 이름")
    data_type: DataType = Field(description="데이터 타입")
    description: str = Field(default="", description="포트 설명")
    
    model_config = {"use_enum_values": True}

class InputPort(Port):
    """입력 포트"""
    required: bool = Field(default=True, description="필수 입력 여부")
    default_value: Optional[Any] = Field(default=None, description="기본값")
    _data: Any = None

    def set_data(self, data: Any):
        self._data = data

    def get_data(self) -> Any:
        return self._data

class OutputPort(Port):
    """출력 포트"""
    _data: Any = None

    def set_data(self, data: Any):
        self._data = data

    def get_data(self) -> Any:
        return self._data
