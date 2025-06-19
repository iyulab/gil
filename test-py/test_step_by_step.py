"""
Gil-Py 단계별 테스트
"""

import sys
import os

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

def test_step_by_step():
    """단계별 테스트"""
    print("🔍 Gil-Py 단계별 테스트")
    
    try:
        print("1. 코어 모듈 임포트...")
        from gil_py.core import GilNode, GilPort, GilDataType
        print("   ✅ 코어 모듈 임포트 성공")
        
        print("2. 데이터 타입 테스트...")
        for dt in GilDataType:
            print(f"   - {dt.name}: {dt.value}")
        print("   ✅ 데이터 타입 테스트 성공")
        
        print("3. 포트 생성 테스트...")
        test_port = GilPort(
            name="test_input",
            data_type=GilDataType.TEXT,
            description="테스트 입력 포트"
        )
        print(f"   ✅ 포트 생성 성공: {test_port.name}")
        
        print("4. OpenAI 커넥터 임포트 테스트...")
        try:
            from gil_py.connectors.openai_connector import GilConnectorOpenAI
            print("   ✅ OpenAI 커넥터 클래스 임포트 성공")
            
            print("5. 커넥터 인스턴스 생성 테스트...")
            connector = GilConnectorOpenAI(api_key="test-key")
            print(f"   ✅ 커넥터 생성 성공: {connector.name}")
            
        except Exception as e:
            print(f"   ❌ 커넥터 관련 오류: {e}")
            import traceback
            traceback.print_exc()
        
        print("6. 이미지 생성기 임포트 테스트...")
        try: 
            from gil_py.generators.image_generator import GilGenImage
            print("   ✅ 이미지 생성기 클래스 임포트 성공")
            
            if 'connector' in locals():
                print("7. 이미지 생성기 인스턴스 생성 테스트...")
                image_gen = GilGenImage(connector=connector)
                print(f"   ✅ 이미지 생성기 생성 성공: {image_gen.name}")
                
        except Exception as e:
            print(f"   ❌ 이미지 생성기 오류: {e}")
            import traceback
            traceback.print_exc()
        
        print("\n🎉 단계별 테스트 완료!")
        
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    test_step_by_step()
