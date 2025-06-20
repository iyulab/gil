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
                description="API 요청 데이터 (선택사항)",
                required=False
            )
        ]
        
        self.output_ports = [
            GilPort(
                name="response",
                data_type=GilDataType.JSON,
                description="API 응답 데이터"
            ),            GilPort(
                name="error",
                data_type=GilDataType.TEXT,
                description="에러 메시지"
            )
        ]
    
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """OpenAI API 요청 실행"""
        request_data = inputs.get("request_data")
        
        # 컨텍스트에 API 호출 정보 기록
        if self.node_context:
            self.node_context.set_variable("api_provider", "openai")
            self.node_context.set_variable("base_url", self.base_url or "https://api.openai.com/v1")
          # request_data가 없으면 연결 정보만 반환
        if not request_data:
            connection_info = {"status": "connected", "client": "available"}
            if self.node_context:
                self.node_context.set_variable("connection_test", True)
            return {
                "response": connection_info,
                "error": None
            }
        
        try:
            # API 엔드포인트별 분기
            endpoint = request_data.get("endpoint", "chat.completions.create")
            client = self.__dict__['client']
              # 컨텍스트에 요청 정보 기록
            if self.node_context:
                self.node_context.set_variable("endpoint", endpoint)
                self.node_context.set_variable("request_params", request_data.get("params", {}))
            
            if endpoint == "chat.completions.create":
                response = await client.chat.completions.create(**request_data.get("params", {}))
            elif endpoint == "images.generate":
                response = await client.images.generate(**request_data.get("params", {}))
            else:
                error_msg = f"Unsupported endpoint: {endpoint}"
                self.log_error(error_msg, "unsupported_endpoint", {"endpoint": endpoint})
                raise ValueError(error_msg)
              # 성공 시 컨텍스트에 응답 정보 기록
            response_dict = response.model_dump() if hasattr(response, 'model_dump') else dict(response)
            if self.node_context:
                self.node_context.set_variable("response_received", True)
                if hasattr(response, 'usage') and response.usage:
                    usage_info = response.usage.model_dump() if hasattr(response.usage, 'model_dump') else dict(response.usage)
                    self.node_context.set_variable("token_usage", usage_info)
                    
                    # Flow 컨텍스트에 토큰 사용량 누적
                    if self.flow_context:
                        total_tokens = self.flow_context.get_shared_data("total_tokens_used", 0)
                        total_tokens += usage_info.get("total_tokens", 0)
                        self.flow_context.set_shared_data("total_tokens_used", total_tokens)
            
            return {
                "response": response_dict,
                "error": None
            }
            
        except Exception as e:
            error_msg = str(e)
            error_details = {
                "endpoint": endpoint,
                "error_type": type(e).__name__,
                "api_provider": "openai"
            }
            
            # 컨텍스트에 에러 정보 기록
            self.log_error(error_msg, "api_error", error_details)
            
            return {
                "response": None,
                "error": error_msg
            }
    
    async def test_connection(self) -> bool:
        """연결 테스트"""
        try:
            client = self.__dict__['client']
            await client.models.list()
            return True
        except Exception:
            return False
