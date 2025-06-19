"""
Gil-Py 간단한 사용 예제
"""

import asyncio
import os
from dotenv import load_dotenv
import sys

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

from gil_py import GilConnectorOpenAI, GilGenImage

# 환경 변수 로드
load_dotenv()


async def simple_example():
    """간단한 사용 예제"""
    print("🎨 Gil-Py 이미지 생성 예제")
    
    # API 키 확인
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ .env 파일에 OPENAI_API_KEY를 설정해주세요.")
        return
    
    try:
        # 1. OpenAI 커넥터 생성
        print("1. OpenAI 커넥터 생성 중...")
        openai_connector = GilConnectorOpenAI(api_key=api_key)
        
        # 2. 이미지 생성 노드 생성
        print("2. 이미지 생성 노드 생성 중...")
        image_gen = GilGenImage(connector=openai_connector)
        
        # 3. 이미지 생성
        print("3. 이미지 생성 중... (시간이 걸릴 수 있습니다)")
        result = await image_gen.generate(
            prompt="A beautiful sunset over mountains with a lake in the foreground",
            size="1024x1024",
            quality="standard"
        )
        
        # 4. 결과 출력
        if result.get("error"):
            print(f"❌ 오류 발생: {result['error']}")
        else:
            images = result.get("images", [])
            print(f"✅ 성공! {len(images)}개 이미지 생성됨")
            for i, img in enumerate(images):
                print(f"   이미지 {i+1} URL: {img['url']}")
        
    except Exception as e:
        print(f"❌ 예외 발생: {e}")


async def advanced_example():
    """고급 사용 예제 - 여러 이미지 생성"""
    print("\n🎨 Gil-Py 고급 예제 - 여러 이미지 생성")
    
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ .env 파일에 OPENAI_API_KEY를 설정해주세요.")
        return
    
    try:
        # 커넥터와 노드 생성
        connector = GilConnectorOpenAI(api_key=api_key)
        image_gen = GilGenImage(connector=connector)
        
        # 여러 프롬프트로 이미지 생성
        prompts = [
            "A cyberpunk city at night with neon lights",
            "A peaceful forest with sunlight filtering through trees",
            "An underwater scene with colorful coral reefs"
        ]
        
        for i, prompt in enumerate(prompts, 1):
            print(f"{i}. '{prompt}' 이미지 생성 중...")
            
            result = await image_gen.generate(
                prompt=prompt,
                size="1024x1024",
                style="vivid"
            )
            
            if result.get("error"):
                print(f"   ❌ 오류: {result['error']}")
            else:
                images = result.get("images", [])
                if images:
                    print(f"   ✅ 생성 완료: {images[0]['url']}")
                else:
                    print("   ❌ 이미지가 생성되지 않았습니다.")
    
    except Exception as e:
        print(f"❌ 예외 발생: {e}")


if __name__ == "__main__":
    asyncio.run(simple_example())
    asyncio.run(advanced_example())
