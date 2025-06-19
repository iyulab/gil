"""
Gil 포트 시스템
"""

from typing import Any, Optional
from pydantic import BaseModel, Field
from .data_types import GilDataType


class GilPort(BaseModel):
    """Gil 노드의 입출력 포트"""
    
    name: str = Field(description="포트 이름")
    data_type: GilDataType = Field(description="데이터 타입")
    required: bool = Field(default=True, description="필수 입력 여부")
    default_value: Optional[Any] = Field(default=None, description="기본값")
    description: str = Field(default="", description="포트 설명")
    
    model_config = {"use_enum_values": True}
    
    def validate_data(self, data: Any) -> bool:
        """데이터 유효성 검증"""
        if data is None and self.required and self.default_value is None:
            return False
        
        # ANY 타입은 모든 데이터 허용
        if self.data_type == GilDataType.ANY:
            return True
            
        # 타입별 검증 로직 추가 가능
        return True
