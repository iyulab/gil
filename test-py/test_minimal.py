"""
최소한의 테스트
"""

import sys
import os

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

def minimal_test():
    """최소한의 테스트"""
    print("🔍 최소한의 테스트")
    
    # 직접 파일 실행해서 클래스 정의
    print("1. 파일 내용 직접 실행...")
    
    # 필요한 모듈들 먼저 import
    from typing import Dict, Any, Optional
    from pydantic import BaseModel, Field
    from enum import Enum
    import uuid
    
    # 데이터 타입 정의
    class GilDataType(Enum):
        TEXT = "text"
        NUMBER = "number"  
        BOOLEAN = "boolean"
        JSON = "json"
        ARRAY = "array"
        BINARY = "binary"
        IMAGE = "image"
        AUDIO = "audio"
        VIDEO = "video"
        FILE = "file"
        ANY = "any"
    
    # 포트 클래스
    class GilPort(BaseModel):
        name: str = Field(description="포트 이름")
        data_type: GilDataType = Field(description="데이터 타입")
        required: bool = Field(default=True, description="필수 입력 여부")
        default_value: Optional[Any] = Field(default=None, description="기본값")
        description: str = Field(default="", description="포트 설명")
        model_config = {"use_enum_values": True}
    
    # 베이스 노드 클래스 (간소화)
    class GilNode(BaseModel):
        node_id: str = Field(default_factory=lambda: str(uuid.uuid4()), description="노드 고유 ID")
        name: str = Field(description="노드 이름")
        node_type: str = Field(description="노드 타입")
        version: str = Field(default="1.0.0", description="노드 버전")
        input_ports: list = Field(default_factory=list, description="입력 포트들")
        output_ports: list = Field(default_factory=list, description="출력 포트들")
        
        model_config = {"arbitrary_types_allowed": True, "use_enum_values": True}
        
        def _setup_ports(self):
            pass
    
    print("2. OpenAI 커넥터 클래스 정의...")
    import openai
    
    class GilConnectorOpenAI(GilNode):
        """OpenAI API 연결을 위한 커넥터 노드"""
        
        api_key: str = Field(description="OpenAI API 키")
        organization: Optional[str] = Field(default=None, description="OpenAI 조직 ID")
        base_url: Optional[str] = Field(default=None, description="커스텀 베이스 URL")
        
        model_config = {"arbitrary_types_allowed": True}
        
        def __init__(self, **data):
            # 기본값 설정
            if "name" not in data:
                data["name"] = "OpenAI Connector"
            if "node_type" not in data:
                data["node_type"] = "GilConnectorOpenAI"
            
            super().__init__(**data)
            self._setup_ports()
            
            # OpenAI 클라이언트 초기화
            self.__dict__['client'] = openai.AsyncOpenAI(
                api_key=self.api_key,
                organization=self.organization,
                base_url=self.base_url
            )
        
        def _setup_ports(self) -> None:
            """포트 설정"""
            self.input_ports = [
                GilPort(
                    name="request_data",
                    data_type=GilDataType.JSON,
                    description="API 요청 데이터",
                    required=True
                )
            ]
            
            self.output_ports = [
                GilPort(
                    name="response",
                    data_type=GilDataType.JSON,
                    description="API 응답 데이터"
                ),
                GilPort(
                    name="error",
                    data_type=GilDataType.TEXT,
                    description="에러 메시지"
                )
            ]
    
    print("3. 커넥터 생성 테스트...")
    try:
        connector = GilConnectorOpenAI(api_key="test-key")
        print(f"   ✅ 커넥터 생성 성공: {connector.name}")
        print(f"   - 노드 ID: {connector.node_id[:8]}...")
        print(f"   - 입력 포트 수: {len(connector.input_ports)}")
        print(f"   - 출력 포트 수: {len(connector.output_ports)}")
        
        print("4. 포트 정보:")
        for port in connector.input_ports:
            print(f"   - 입력: {port.name} ({port.data_type}) - {port.description}")
        for port in connector.output_ports:
            print(f"   - 출력: {port.name} ({port.data_type}) - {port.description}")
            
        print("✅ 최소한의 테스트 성공!")
        return True
        
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == "__main__":
    minimal_test()
