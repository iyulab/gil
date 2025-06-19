"""
Gil-Py 실제 사용 예제 (API 키 없이)
"""

import sys
import os
import asyncio

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

# 필요한 모듈들 직접 import
from typing import Dict, Any, Optional, Literal
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
    
    def validate_data(self, data: Any) -> bool:
        """데이터 유효성 검증"""
        if data is None and self.required and self.default_value is None:
            return False
        if self.data_type == GilDataType.ANY:
            return True
        return True

# 베이스 노드 클래스
class GilNode(BaseModel):
    node_id: str = Field(default_factory=lambda: str(uuid.uuid4()), description="노드 고유 ID")
    name: str = Field(description="노드 이름")
    node_type: str = Field(description="노드 타입")
    version: str = Field(default="1.0.0", description="노드 버전")
    input_ports: list = Field(default_factory=list, description="입력 포트들")
    output_ports: list = Field(default_factory=list, description="출력 포트들")
    is_running: bool = Field(default=False, description="실행 중 여부")
    last_execution_time: Optional[float] = Field(default=None, description="마지막 실행 시간")
    
    model_config = {"arbitrary_types_allowed": True, "use_enum_values": True}
    
    def __init__(self, **data):
        super().__init__(**data)
        self._setup_ports()
    
    def _setup_ports(self):
        """포트 설정을 정의하는 메서드"""
        pass
    
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
    
    async def execute(self, inputs: Dict[str, Any]) -> Dict[str, Any]:
        """노드 실행 로직"""
        return {}

# OpenAI 커넥터 (모의)
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
        """OpenAI API 요청 실행 (모의)"""
        request_data = inputs["request_data"]
        endpoint = request_data.get("endpoint", "images.generate")
        
        # 모의 응답 생성
        if endpoint == "images.generate":
            mock_response = {
                "data": [
                    {
                        "url": "https://example.com/generated_image.jpg",
                        "revised_prompt": request_data.get("params", {}).get("prompt", "A beautiful image")
                    }
                ]
            }
            return {
                "response": mock_response,
                "error": None
            }
        else:
            return {
                "response": None,
                "error": "Unsupported endpoint in demo mode"
            }

# 이미지 생성기
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
                description="이미지 크기",
                default_value="1024x1024",
                required=False
            ),
            GilPort(
                name="quality",
                data_type=GilDataType.TEXT,
                description="이미지 품질",
                default_value="standard",
                required=False
            ),
            GilPort(
                name="style",
                data_type=GilDataType.TEXT,
                description="이미지 스타일",
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


async def demo_workflow():
    """데모 워크플로우"""
    print("🎨 Gil-Py 이미지 생성 데모 (모의 모드)")
    print("="*50)
    
    # 1. 커넥터 생성
    print("1. OpenAI 커넥터 생성...")
    connector = GilConnectorOpenAI(api_key="demo-key")
    print(f"   ✅ 생성됨: {connector.name} (ID: {connector.node_id[:8]}...)")
    
    # 2. 이미지 생성기 생성  
    print("2. 이미지 생성기 생성...")
    image_gen = GilGenImage(connector=connector)
    print(f"   ✅ 생성됨: {image_gen.name} (ID: {image_gen.node_id[:8]}...)")
    
    # 3. 노드 정보 출력
    print(f"\n📊 노드 정보:")
    print(f"   커넥터 포트: {len(connector.input_ports)} 입력, {len(connector.output_ports)} 출력")
    print(f"   생성기 포트: {len(image_gen.input_ports)} 입력, {len(image_gen.output_ports)} 출력")
    
    # 4. 이미지 생성 테스트
    print(f"\n🎨 이미지 생성 테스트:")
    test_prompts = [
        "A serene mountain landscape at sunset",
        "A futuristic city with flying cars",
        "A cozy library with warm lighting"
    ]
    
    for i, prompt in enumerate(test_prompts, 1):
        print(f"\n   {i}. 프롬프트: '{prompt}'")
        
        result = await image_gen.generate(
            prompt=prompt,
            size="1024x1024",
            quality="standard"
        )
        
        if result.get("error"):
            print(f"      ❌ 오류: {result['error']}")
        else:
            images = result.get("images", [])
            if images:
                print(f"      ✅ 성공! {len(images)}개 이미지 생성됨")
                for j, img in enumerate(images):
                    print(f"         이미지 {j+1}: {img['url']}")
                    print(f"         수정된 프롬프트: {img['revised_prompt']}")
            else:
                print("      ⚠️  이미지가 생성되지 않았습니다")
    
    # 5. 실행 통계
    print(f"\n📈 실행 통계:")
    print(f"   커넥터 마지막 실행: {connector.last_execution_time}")
    print(f"   생성기 마지막 실행: {image_gen.last_execution_time}")
    
    print(f"\n🎉 데모 완료!")


if __name__ == "__main__":
    asyncio.run(demo_workflow())
