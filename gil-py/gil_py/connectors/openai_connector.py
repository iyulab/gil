"""
OpenAI API 커넥터
"""

from typing import Dict, Any, Optional
from pydantic import Field
import openai
from ..core import GilNode, GilPort, GilDataType


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
    
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """OpenAI API 요청 실행"""
        request_data = inputs["request_data"]
        
        try:
            # API 엔드포인트별 분기
            endpoint = request_data.get("endpoint", "chat.completions.create")
            client = self.__dict__['client']
            
            if endpoint == "chat.completions.create":
                response = await client.chat.completions.create(**request_data.get("params", {}))
            elif endpoint == "images.generate":
                response = await client.images.generate(**request_data.get("params", {}))
            else:
                raise ValueError(f"Unsupported endpoint: {endpoint}")
            
            return {
                "response": response.model_dump() if hasattr(response, 'model_dump') else dict(response),
                "error": None
            }
            
        except Exception as e:
            return {
                "response": None,
                "error": str(e)
            }
    
    async def test_connection(self) -> bool:
        """연결 테스트"""
        try:
            client = self.__dict__['client']
            await client.models.list()
            return True
        except Exception:
            return False
