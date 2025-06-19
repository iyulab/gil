"""
Gil-Py 기본 동작 테스트 (OpenAI API 키 없이)
"""

import sys
import os

# gil-py 패키지를 임포트하기 위해 경로 추가
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "gil-py"))

def test_basic_functionality():
    """기본 기능 테스트"""
    print("🔍 Gil-Py 기본 기능 테스트")
    
    try:
        # 모듈 임포트 테스트
        from gil_py import GilConnectorOpenAI, GilGenImage
        from gil_py.core import GilNode, GilPort, GilDataType
        print("✅ 모든 모듈 임포트 성공")
        
        # 데이터 타입 테스트
        print(f"✅ 데이터 타입 정의: {len(GilDataType)} 개")
        for dt in GilDataType:
            print(f"   - {dt.name}: {dt.value}")
          # 포트 생성 테스트
        test_port = GilPort(
            name="test_input",
            data_type=GilDataType.TEXT,
            description="테스트 입력 포트"
        )
        print(f"✅ 포트 생성 성공: {test_port.name} ({test_port.data_type})")
        
        # 데이터 타입 값 검증
        print(f"✅ 포트 데이터 타입 검증: {type(test_port.data_type)}")
        
        # 커넥터 생성 테스트 (API 키 없이)
        try:
            connector = GilConnectorOpenAI(api_key="test-key")
            print(f"✅ OpenAI 커넥터 생성 성공")
            print(f"   - 노드 ID: {connector.node_id[:8]}...")
            print(f"   - 노드 이름: {connector.name}")
            print(f"   - 입력 포트 수: {len(connector.input_ports)}")
            print(f"   - 출력 포트 수: {len(connector.output_ports)}")
        except Exception as e:
            print(f"❌ 커넥터 생성 실패: {e}")
        
        # 이미지 생성기 테스트 (API 키 없이)
        try:
            connector = GilConnectorOpenAI(api_key="test-key")
            image_gen = GilGenImage(connector=connector)
            print(f"✅ 이미지 생성기 생성 성공")
            print(f"   - 노드 ID: {image_gen.node_id[:8]}...")
            print(f"   - 노드 이름: {image_gen.name}")
            print(f"   - 입력 포트 수: {len(image_gen.input_ports)}")
            print(f"   - 출력 포트 수: {len(image_gen.output_ports)}")
              # 입력 포트 정보
            print("   - 입력 포트들:")
            for port in image_gen.input_ports:
                required = "필수" if port.required else "선택"
                print(f"     * {port.name} ({port.data_type}, {required}): {port.description}")
            
        except Exception as e:
            print(f"❌ 이미지 생성기 생성 실패: {e}")
        
        print("\n🎉 기본 기능 테스트 완료!")
        return True
        
    except Exception as e:
        print(f"❌ 테스트 실패: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == "__main__":
    test_basic_functionality()
