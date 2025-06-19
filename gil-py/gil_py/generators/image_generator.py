"""
이미지 생성 노드
"""

from typing import Dict, Any, Optional, Literal
from pydantic import Field
from ..core import GilNode, GilPort, GilDataType
from ..connectors import GilConnectorOpenAI


class GilGenImage(GilNode):
    """이미지 생성을 위한 노드"""
    
    connector: GilConnectorOpenAI = Field(description="OpenAI 커넥터")
    
    model_config = {"arbitrary_types_allowed": True}
    
    def __init__(self, **data):
        # 기본값 설정
        if "name" not in data:
            data["name"] = "Image Generator"
        if "node_type" not in data:
            data["node_type"] = "GilGenImage"
        
        super().__init__(**data)
    
    def _setup_ports(self) -> None:
        """포트 설정"""
        self.input_ports = [
            GilPort(
                name="prompt",
                data_type=GilDataType.TEXT,
                description="이미지 생성 프롬프트",
                required=True
            ),
            GilPort(
                name="size",
                data_type=GilDataType.TEXT,
                description="이미지 크기 (1024x1024, 1024x1792, 1792x1024)",
                default_value="1024x1024",
                required=False
            ),
            GilPort(
                name="quality",
                data_type=GilDataType.TEXT,
                description="이미지 품질 (standard, hd)",
                default_value="standard",
                required=False
            ),
            GilPort(
                name="style",
                data_type=GilDataType.TEXT,
                description="이미지 스타일 (vivid, natural)",
                default_value="vivid",
                required=False
            ),
            GilPort(
                name="n",
                data_type=GilDataType.NUMBER,
                description="생성할 이미지 수",
                default_value=1,
                required=False
            )
        ]
        
        self.output_ports = [
            GilPort(
                name="images",
                data_type=GilDataType.ARRAY,
                description="생성된 이미지 정보 배열"
            ),
            GilPort(
                name="error",
                data_type=GilDataType.TEXT,
                description="에러 메시지"
            )
        ]
    
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """이미지 생성 실행"""
        prompt = inputs["prompt"]
        size = inputs.get("size", "1024x1024")
        quality = inputs.get("quality", "standard")
        style = inputs.get("style", "vivid")
        n = inputs.get("n", 1)
        
        # OpenAI API 요청 데이터 구성
        request_data = {
            "endpoint": "images.generate",
            "params": {
                "model": "dall-e-3",
                "prompt": prompt,
                "size": size,
                "quality": quality,
                "style": style,
                "n": n
            }
        }
        
        # 커넥터를 통해 API 호출
        result = await self.connector.run({"request_data": request_data})
        
        if result.get("error"):
            return {
                "images": [],
                "error": result["error"]
            }
        
        # 응답 데이터 처리
        response = result.get("response", {})
        images = []
        
        if "data" in response:
            for img_data in response["data"]:
                images.append({
                    "url": img_data.get("url"),
                    "revised_prompt": img_data.get("revised_prompt", prompt)
                })
        
        return {
            "images": images,
            "error": None
        }
    
    async def generate(
        self,
        prompt: str,
        size: Literal["1024x1024", "1024x1792", "1792x1024"] = "1024x1024",
        quality: Literal["standard", "hd"] = "standard",
        style: Literal["vivid", "natural"] = "vivid",
        n: int = 1
    ) -> Dict[str, Any]:
        """편의 메서드: 이미지 생성"""
        inputs = {
            "prompt": prompt,
            "size": size,
            "quality": quality,
            "style": style,
            "n": n
        }
        
        return await self.run(inputs)
