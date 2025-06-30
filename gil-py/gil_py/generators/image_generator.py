"""
이미지 생성 노드
"""

from typing import Dict, Any, Optional, Literal
from pydantic import Field
import openai
from ..core.node import Node
from ..core.port import InputPort, OutputPort
from ..core.data_types import DataType
from ..connectors.openai_connector import OpenAIConnector


class ImageGenerator(Node):
    """이미지 생성을 위한 노드"""
    
    connector: OpenAIConnector = Field(description="OpenAI 커넥터")
    
    model_config = {"arbitrary_types_allowed": True}
    
    def __init__(self, node_id: str, config: dict = None):
        super().__init__(node_id, config)
        self.connector = self.config.get("connector")

        self.add_input_port(InputPort("prompt", DataType.TEXT, description="이미지 생성 프롬프트", required=True))
        self.add_input_port(InputPort("size", DataType.TEXT, description="이미지 크기 (1024x1024, 1024x1792, 1792x1024)", default_value="1024x1024", required=False))
        self.add_input_port(InputPort("quality", DataType.TEXT, description="이미지 품질 (standard, hd)", default_value="standard", required=False))
        self.add_input_port(InputPort("style", DataType.TEXT, description="이미지 스타일 (vivid, natural)", default_value="vivid", required=False))
        self.add_input_port(InputPort("n", DataType.NUMBER, description="생성할 이미지 수", default_value=1, required=False))

        self.add_output_port(OutputPort("images", DataType.ARRAY, description="생성된 이미지 정보 배열"))
        self.add_output_port(OutputPort("error", DataType.TEXT, description="에러 메시지"))

    async def execute(self):
        """이미지 생성 실행"""
        # 테스트용 모의 이미지 확인
        if self.config.get("mock_result") and "test_images" in self.config:
            self.get_output_port("images").set_data(self.config["test_images"])
            self.get_output_port("error").set_data(None)
            return
        
        prompt = self.get_input_port("prompt").get_data()
        size = self.get_input_port("size").get_data()
        quality = self.get_input_port("quality").get_data()
        style = self.get_input_port("style").get_data()
        n = self.get_input_port("n").get_data()
        
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
        await self.connector.execute()
        result = {
            "response": self.connector.get_output_port("response").get_data(),
            "error": self.connector.get_output_port("error").get_data()
        }
        
        if result.get("error"):
            self.get_output_port("images").set_data([])
            self.get_output_port("error").set_data(result["error"])
            return
        
        # 응답 데이터 처리
        response = result.get("response", {})
        images = []
        
        if "data" in response:
            for img_data in response["data"]:
                images.append({
                    "url": img_data.get("url"),
                    "revised_prompt": img_data.get("revised_prompt", prompt)
                })
        
        self.get_output_port("images").set_data(images)
        self.get_output_port("error").set_data(None)
        return
    
    async def generate(
        self,
        prompt: str,
        size: Literal["1024x1024", "1024x1792", "1792x1024"] = "1024x1024",
        quality: Literal["standard", "hd"] = "standard",
        style: Literal["vivid", "natural"] = "vivid",
        n: int = 1
    ) -> None:
        """편의 메서드: 이미지 생성"""
        self.get_input_port("prompt").set_data(prompt)
        self.get_input_port("size").set_data(size)
        self.get_input_port("quality").set_data(quality)
        self.get_input_port("style").set_data(style)
        self.get_input_port("n").set_data(n)
        
        await self.run()
