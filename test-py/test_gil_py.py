"""
Gil-Py 라이브러리 테스트
"""

import asyncio
import os
from dotenv import load_dotenv
import sys
import os

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

from gil_py import GilConnectorOpenAI, GilGenImage

# 환경 변수 로드
load_dotenv()


async def test_openai_connector():
    """OpenAI 커넥터 테스트"""
    print("=== OpenAI 커넥터 테스트 ===")
    
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY 환경변수가 설정되지 않았습니다.")
        return False
    
    try:
        # 커넥터 생성
        connector = GilConnectorOpenAI(api_key=api_key)
        
        # 연결 테스트
        is_connected = await connector.test_connection()
        if is_connected:
            print("✅ OpenAI API 연결 성공")
            return True
        else:
            print("❌ OpenAI API 연결 실패")
            return False
            
    except Exception as e:
        print(f"❌ 커넥터 테스트 중 오류: {e}")
        return False


async def test_image_generation():
    """이미지 생성 노드 테스트"""
    print("\n=== 이미지 생성 노드 테스트 ===")
    
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY 환경변수가 설정되지 않았습니다.")
        return False
    
    try:
        # 커넥터 및 이미지 생성 노드 생성
        connector = GilConnectorOpenAI(api_key=api_key)
        image_gen = GilGenImage(connector=connector)
        
        # 이미지 생성 테스트
        print("🎨 이미지 생성 중...")
        result = await image_gen.generate(
            prompt="A serene landscape with mountains and a lake at sunset",
            size="1024x1024",
            quality="standard",
            style="vivid"
        )
        
        if result.get("error"):
            print(f"❌ 이미지 생성 실패: {result['error']}")
            return False
        
        images = result.get("images", [])
        if images:
            print(f"✅ 이미지 생성 성공! {len(images)}개 이미지 생성됨")
            for i, img in enumerate(images):
                print(f"  - 이미지 {i+1}: {img['url']}")
                print(f"  - 수정된 프롬프트: {img['revised_prompt']}")
            return True
        else:
            print("❌ 이미지 생성 결과가 없습니다.")
            return False
            
    except Exception as e:
        print(f"❌ 이미지 생성 테스트 중 오류: {e}")
        return False


async def test_node_workflow():
    """노드 워크플로우 테스트"""
    print("\n=== 노드 워크플로우 테스트 ===")
    
    api_key = os.getenv("OPENAI_API_KEY")
    if not api_key:
        print("❌ OPENAI_API_KEY 환경변수가 설정되지 않았습니다.")
        return False
    
    try:
        # 노드들 생성
        connector = GilConnectorOpenAI(api_key=api_key)
        image_gen = GilGenImage(connector=connector)
        
        # 노드 정보 출력
        print(f"📦 노드 생성 완료:")
        print(f"  - 커넥터: {connector.name} (ID: {connector.node_id[:8]}...)")
        print(f"  - 이미지 생성기: {image_gen.name} (ID: {image_gen.node_id[:8]}...)")
        
        # 포트 정보 출력
        print(f"\n🔌 이미지 생성기 포트 정보:")
        print(f"  입력 포트:")
        for port in image_gen.input_ports:
            print(f"    - {port.name} ({port.data_type.value}): {port.description}")
        print(f"  출력 포트:")
        for port in image_gen.output_ports:
            print(f"    - {port.name} ({port.data_type.value}): {port.description}")
        
        # 워크플로우 실행
        print(f"\n⚡ 워크플로우 실행:")
        inputs = {
            "prompt": "A futuristic city with flying cars and neon lights",
            "size": "1024x1024",
            "quality": "standard"
        }
        
        print(f"  입력: {inputs}")
        result = await image_gen.run(inputs)
        
        if result.get("error"):
            print(f"❌ 워크플로우 실행 실패: {result['error']}")
            return False
        
        print(f"✅ 워크플로우 실행 성공!")
        print(f"  실행 시간: {image_gen.last_execution_time}")
        print(f"  결과: {len(result.get('images', []))}개 이미지 생성")
        
        return True
        
    except Exception as e:
        print(f"❌ 워크플로우 테스트 중 오류: {e}")
        return False


async def main():
    """메인 테스트 함수"""
    print("🚀 Gil-Py 라이브러리 테스트 시작\n")
    
    tests = [
        ("OpenAI 커넥터", test_openai_connector),
        ("이미지 생성", test_image_generation),
        ("노드 워크플로우", test_node_workflow),
    ]
    
    results = []
    for test_name, test_func in tests:
        try:
            result = await test_func()
            results.append((test_name, result))
        except Exception as e:
            print(f"❌ {test_name} 테스트 중 예외 발생: {e}")
            results.append((test_name, False))
    
    # 결과 요약
    print("\n" + "="*50)
    print("📊 테스트 결과 요약")
    print("="*50)
    
    passed = 0
    for test_name, result in results:
        status = "✅ 통과" if result else "❌ 실패"
        print(f"{test_name}: {status}")
        if result:
            passed += 1
    
    print(f"\n총 {len(results)}개 테스트 중 {passed}개 통과")
    
    if passed == len(results):
        print("🎉 모든 테스트가 통과했습니다!")
    else:
        print("⚠️  일부 테스트가 실패했습니다. 확인이 필요합니다.")


if __name__ == "__main__":
    asyncio.run(main())
