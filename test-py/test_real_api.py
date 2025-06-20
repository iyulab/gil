"""
Gil-Py 실제 OpenAI API 테스트
"""

import sys
import os
import asyncio
from dotenv import load_dotenv

# 환경 변수 로드
load_dotenv()

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

# 필요한 모듈들 직접 import
from typing import Dict, Any, Optional, Literal
from pydantic import BaseModel, Field
from enum import Enum
import uuid
import openai

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

# 실제 OpenAI 커넥터
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
        
        # 실제 OpenAI 클라이언트 초기화
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
        """실제 OpenAI API 요청 실행"""
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
            models = await client.models.list()
            return True
        except Exception as e:
            print(f"연결 테스트 실패: {e}")
            return False

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


async def test_real_api():
    """실제 OpenAI API 테스트"""
    print("🚀 Gil-Py 실제 OpenAI API 테스트")
    print("="*60)
    
    # API 키 확인
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY 환경변수가 설정되지 않았습니다.")
        print("   .env 파일에 API 키를 설정해주세요.")
        return False
    
    if api_key.startswith("your-") or api_key == "test-key":
        print("❌ 실제 OpenAI API 키가 필요합니다.")
        print("   .env 파일에 실제 API 키를 설정해주세요.")
        return False
    
    print(f"✅ API 키 확인됨: {api_key[:20]}...")
    
    try:
        # 1. 커넥터 생성 및 연결 테스트
        print("\n1. OpenAI 커넥터 생성 및 연결 테스트...")
        connector = GilConnectorOpenAI(api_key=api_key)
        print(f"   📦 커넥터 생성: {connector.name} (ID: {connector.node_id[:8]}...)")
        
        # 연결 테스트
        is_connected = await connector.test_connection()
        if is_connected:
            print("   ✅ OpenAI API 연결 성공!")
        else:
            print("   ❌ OpenAI API 연결 실패")
            return False
        
        # 2. 이미지 생성기 생성
        print("\n2. 이미지 생성기 생성...")
        image_gen = GilGenImage(connector=connector)
        print(f"   📦 이미지 생성기 생성: {image_gen.name} (ID: {image_gen.node_id[:8]}...)")
        
        # 3. 실제 이미지 생성 테스트
        print("\n3. 실제 이미지 생성 테스트...")
        test_prompts = [
            {
                "prompt": "A beautiful sunset over a calm lake with mountains in the background",
                "description": "고요한 호수와 산의 일몰"
            },
            {
                "prompt": "A cute robot sitting in a flower garden",
                "description": "꽃밭에 앉은 귀여운 로봇"
            }
        ]
        
        for i, test_case in enumerate(test_prompts, 1):
            print(f"\n   테스트 {i}: {test_case['description']}")
            print(f"   프롬프트: '{test_case['prompt']}'")
            print("   🎨 이미지 생성 중... (30초 정도 소요될 수 있습니다)")
            
            start_time = asyncio.get_event_loop().time()
            
            result = await image_gen.generate(
                prompt=test_case['prompt'],
                size="1024x1024",
                quality="standard",
                style="vivid"
            )
            
            end_time = asyncio.get_event_loop().time()
            duration = round(end_time - start_time, 2)
            
            if result.get("error"):
                print(f"   ❌ 실패: {result['error']}")
            else:
                images = result.get("images", [])
                if images:
                    print(f"   ✅ 성공! {len(images)}개 이미지 생성됨 (소요시간: {duration}초)")
                    for j, img in enumerate(images):
                        print(f"      이미지 {j+1} URL: {img['url']}")
                        if img['revised_prompt'] != test_case['prompt']:
                            print(f"      수정된 프롬프트: {img['revised_prompt']}")
                else:
                    print("   ❌ 이미지가 생성되지 않았습니다.")
        
        # 4. 실행 통계
        print(f"\n📊 실행 통계:")
        print(f"   커넥터 마지막 실행: {connector.last_execution_time}")
        print(f"   생성기 마지막 실행: {image_gen.last_execution_time}")
        
        print(f"\n🎉 실제 API 테스트 완료!")
        return True
        
    except Exception as e:
        print(f"❌ 테스트 중 오류 발생: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == "__main__":
    asyncio.run(test_real_api())
